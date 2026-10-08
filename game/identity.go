package game

import (
	"context"
	"encoding/json"
	"regexp"
	"strings"
	"sync"
	"time"
	"unicode/utf8"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/earth-two/identity"
	"github.com/struckchure/earth-two/internals/account"
)

const identityFields = 4

type identityPanel struct {
	fields         [identityFields]string // passphrase, transfer path, email, display name
	host, database string
	backup         []byte
	key            *identity.Key
	session        *account.Session
	message        string
	failed, busy   bool
	ctx            context.Context
	cancel         context.CancelFunc
	jobs           sync.WaitGroup
	results        chan identityResult
	pingResults    chan latencyResult
	pingPending    bool
	nextPing       time.Time
	latency        time.Duration
}
type latencyResult struct {
	session *account.Session
	latency time.Duration
	err     error
}
type identityResult struct {
	backup      []byte
	key         *identity.Key
	session     *account.Session
	email       *string
	displayName *string
	message     string
	err         error
}

func identityBackupID(data []byte) string {
	var b struct {
		Format    string `json:"format"`
		Version   int    `json:"version"`
		ID        string `json:"account_id"`
		PublicKey []byte `json:"public_key"`
	}
	if json.Unmarshal(data, &b) != nil || b.Format != "earth-two-key" || b.Version != 1 {
		return ""
	}
	id, err := identity.AccountID(b.PublicKey)
	if err != nil || id != b.ID || !regexp.MustCompile(`^e2_[0-9a-f]{64}$`).MatchString(id) {
		return ""
	}
	return id
}

func newIdentityPanel() *identityPanel {
	ctx, cancel := context.WithCancel(context.Background())
	p := &identityPanel{ctx: ctx, cancel: cancel, results: make(chan identityResult, 1), pingResults: make(chan latencyResult, 2), message: "Your key is your account. Email is optional."}
	p.fields[1] = defaultIdentityTransfer()
	p.host, p.database = identityNetworkDefaults()
	var err error
	p.backup, err = loadIdentityBackup()
	if err != nil {
		p.message, p.failed = err.Error(), true
	}
	if len(p.backup) > 0 && identityBackupID(p.backup) == "" {
		p.message, p.failed = "The saved key is damaged. Import a valid backup.", true
	}
	return p
}

func (p *identityPanel) close() {
	p.cancel()
	p.jobs.Wait()
	if p.session != nil {
		p.session.Close()
	}
	// The game releases its reference to the unlocked key on close or lock.
	p.key = nil
	p.fields[0] = ""
	select {
	case r := <-p.results:
		if r.session != nil {
			r.session.Close()
		}
	default:
	}
}

func (p *identityPanel) poll() {
	defer p.pollLatency()
	select {
	case r := <-p.results:
		p.busy = false
		if r.err != nil {
			p.message, p.failed = r.err.Error(), true
			return
		}
		if r.backup != nil {
			p.backup = r.backup
		}
		if r.key != nil {
			if p.session != nil {
				p.session.Close()
				p.session = nil
			}
			p.key = r.key
			p.fields[0] = ""
		}
		if r.session != nil {
			if p.session != nil {
				p.session.Close()
			}
			p.session = r.session
			p.latency = 0
			p.nextPing = time.Time{}
			p.pingPending = false
		}
		if r.email != nil {
			p.fields[2] = *r.email
		}
		if r.displayName != nil && p.fields[3] == "" {
			p.fields[3] = *r.displayName
		}
		p.message, p.failed = r.message, false
	default:
	}
}

func (p *identityPanel) pollLatency() {
	select {
	case result := <-p.pingResults:
		if result.session == p.session {
			p.pingPending = false
			if result.err == nil {
				p.latency = result.latency
			} else {
				p.latency = 0
			}
		}
	default:
	}
	if p.session == nil || !p.session.IsActive() {
		p.latency = 0
		return
	}
	if p.pingPending || time.Now().Before(p.nextPing) {
		return
	}
	session := p.session
	p.pingPending = true
	p.nextPing = time.Now().Add(3 * time.Second)
	p.jobs.Add(1)
	go func() {
		defer p.jobs.Done()
		ctx, cancel := context.WithTimeout(p.ctx, 2*time.Second)
		defer cancel()
		latency, err := session.Ping(ctx)
		select {
		case p.pingResults <- latencyResult{session: session, latency: latency, err: err}:
		case <-p.ctx.Done():
		}
	}()
}

func (p *identityPanel) start(job func(context.Context) identityResult) {
	if p.busy {
		return
	}
	p.busy, p.failed, p.message = true, false, "Working…"
	p.jobs.Add(1)
	go func() {
		defer p.jobs.Done()
		ctx, cancel := context.WithTimeout(p.ctx, 30*time.Second)
		defer cancel()
		result := job(ctx)
		p.results <- result
	}()
}

func (p *identityPanel) enabled(a action) bool {
	if a == actBack {
		return true
	}
	if p.busy {
		return false
	}
	switch a {
	case actIdentityCreate:
		return len(p.backup) == 0 && p.fields[0] != ""
	case actIdentityUnlock:
		return len(p.backup) > 0 && p.fields[0] != ""
	case actIdentityImport:
		return p.fields[0] != ""
	case actIdentityExport:
		return len(p.backup) > 0
	case actIdentityConnect, actIdentityLock:
		return p.key != nil
	case actIdentityEmail, actIdentityName:
		return p.key != nil && p.session != nil && p.session.IsActive()
	}
	return false
}

func (p *identityPanel) act(a action) {
	if !p.enabled(a) {
		p.message, p.failed = "Enter your key passphrase, or unlock your account first.", true
		return
	}
	fields, backup, key, session := p.fields, append([]byte(nil), p.backup...), p.key, p.session
	switch a {
	case actIdentityCreate:
		p.start(func(_ context.Context) identityResult {
			k, err := identity.Generate()
			if err != nil {
				return identityResult{err: err}
			}
			data, err := k.Export(fields[0])
			if err == nil {
				err = saveIdentityBackup(data)
			}
			return identityResult{backup: data, key: k, err: err, message: "Identity created. Export a backup, then connect when ready."}
		})
	case actIdentityUnlock:
		p.start(func(_ context.Context) identityResult {
			k, err := identity.Import(backup, fields[0])
			return identityResult{key: k, err: err, message: "Key unlocked. Connect to authorize your account."}
		})
	case actIdentityImport:
		p.start(func(ctx context.Context) identityResult {
			data, err := readIdentityTransfer(p.ctx, fields[1])
			if err != nil {
				return identityResult{err: err}
			}
			k, err := identity.Import(data, fields[0])
			if err == nil {
				err = saveIdentityBackup(data)
			}
			return identityResult{backup: data, key: k, err: err, message: "Identity imported. Your account ID is unchanged."}
		})
	case actIdentityExport:
		p.start(func(_ context.Context) identityResult {
			err := exportIdentityTransfer(backup, fields[1])
			return identityResult{err: err, message: "Encrypted key exported. Keep it and its passphrase safe."}
		})
	case actIdentityConnect:
		p.start(func(ctx context.Context) identityResult {
			s, err := account.Connect(p.ctx, p.host, p.database, key)
			if err != nil {
				return identityResult{err: err}
			}
			// The connection must outlive this operation's timeout. Native
			// Connect receives the panel lifetime; the wait is bounded below.
			d, err := s.Details()
			if err != nil {
				s.Close()
				return identityResult{err: err}
			}
			return identityResult{session: s, email: &d.Email, displayName: &d.DisplayName, message: "Account connected. Your key authorizes changes."}
		})
	case actIdentityEmail:
		p.start(func(ctx context.Context) identityResult {
			err := session.SetEmail(ctx, fields[2])
			return identityResult{err: err, message: "Optional contact email saved. It cannot recover your key."}
		})
	case actIdentityName:
		p.start(func(ctx context.Context) identityResult {
			err := session.SetDisplayName(ctx, fields[3])
			return identityResult{err: err, message: "Display name saved. Your signing key remains your identity."}
		})
	case actIdentityLock:
		if session != nil {
			session.Close()
		}
		p.session, p.key = nil, nil
		p.fields[0] = ""
		p.message, p.failed = "Key locked. You can still export its encrypted backup.", false
	}
}

func (p *identityPanel) edit(focus int) {
	if p.busy || focus < 0 || focus >= identityFields || (focus == 1 && !canQuit) {
		return
	}
	value := &p.fields[focus]
	if rl.IsKeyPressed(rl.KeyBackspace) && *value != "" {
		_, n := utf8.DecodeLastRuneInString(*value)
		*value = (*value)[:len(*value)-n]
	}
	modifier := rl.IsKeyDown(rl.KeyLeftControl) || rl.IsKeyDown(rl.KeyRightControl) || rl.IsKeyDown(rl.KeyLeftSuper) || rl.IsKeyDown(rl.KeyRightSuper)
	if modifier && rl.IsKeyPressed(rl.KeyA) {
		*value = ""
	}
	limits := [identityFields]int{4096, 1024, 254, 512}
	if pasted := identityPasteText(modifier && rl.IsKeyPressed(rl.KeyV)); pasted != "" {
		for _, ch := range pasted {
			if ch >= 32 && ch != 127 && len(*value)+utf8.RuneLen(ch) <= limits[focus] {
				*value += string(ch)
			}
		}
	}
	for ch := rl.GetCharPressed(); ch != 0; ch = rl.GetCharPressed() {
		if !modifier && ch >= 32 && ch != 127 && utf8.ValidRune(rune(ch)) && len(*value)+utf8.RuneLen(rune(ch)) <= limits[focus] {
			*value += string(rune(ch))
		}
	}
}

func drawIdentity(p painter, l layout, focus int, panel *identityPanel) {
	p.s = l.sc
	p.paper(l.panel)
	pad := p.px(28)
	p.formHeader(rl.Rectangle{X: l.panel.X + pad, Y: l.panel.Y + pad, Width: l.panel.Width - 2*pad, Height: p.px(100)}, "THE EXCHANGE · LANDFALL", "FORM I-1", "Your identity", 28)
	id := identityBackupID(panel.backup)
	if id == "" {
		id = "No identity saved"
	}
	state := "Locked"
	if panel.key != nil {
		state = "Unlocked"
	}
	if panel.session != nil && panel.session.IsActive() {
		state = "Connected"
	}
	p.textIn(state+" · "+id, l.account, 12, typed, ledgerInk, left)
	labels := [identityFields]string{"KEY PASSPHRASE", "KEY FILE (IMPORT / EXPORT)", "OPTIONAL CONTACT EMAIL", "DISPLAY NAME (ANY NAME; DOES NOT HAVE TO BE UNIQUE)"}
	for i, r := range l.rows {
		if i == focus {
			rl.DrawRectangleRec(r, rl.NewColor(225, 219, 194, 255))
		}
		value := panel.fields[i]
		if i == 0 {
			value = strings.Repeat("•", min(32, utf8.RuneCountInString(value)))
		}
		if value == "" {
			value = "—"
		}
		p.field(inset(r, p.px(8), 0), labels[i], value)
	}
	for i, it := range items(identityScreen) {
		r := l.buttons[i]
		p.tickChoice(r, it.label, focus == identityFields+i)
		if !panel.enabled(it.act) {
			rl.DrawRectangleRec(r, rl.NewColor(250, 245, 232, 145))
		}
	}
	colour := ledgerMuted
	if panel.failed {
		colour = stampRed
	}
	p.textIn(panel.message, l.hint, 12, typed, colour, left)
	p.textIn("Only your private key controls this account. Losing every backup means losing access.", rl.Rectangle{X: l.hint.X, Y: l.hint.Y + p.px(28), Width: l.hint.Width, Height: p.px(28)}, 11, typed, ledgerMuted, left)
}
