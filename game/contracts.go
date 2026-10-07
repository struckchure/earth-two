package game

import (
	"fmt"
	"math"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/earth-two/vehicle"
	"github.com/struckchure/earth-two/world"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/transform"
)

// The first contract is sponsored day labour, open to an Unlisted arrival.
// Accepting issues its sealed filing; handing it to the Exchange credits
// Ada's passage debt once. Like the rest of play, this record lasts this run.
type contractState uint8

const (
	contractAvailable contractState = iota
	contractAccepted
	contractDelivered
)

const (
	passageDebt      = 2000
	firstContractPay = 150
)

type contracts struct {
	state           contractState
	balance         int // spendable marks; debt credits are accounted separately
	debt            int
	ongoing         *contractRecord // one work order at a time
	completed       []contractRecord
	offer, delivery rl.Vector3 // feet in front of the actual placed pieces
}

// Completed records are copies of the accepted terms, with the actual
// settlement. Keeping them separate frees the ongoing slot on delivery.
type contractRecord struct {
	id, title, poster, objective string
	pay, debtCredit              int
}

func firstWorkOrder() contractRecord {
	return contractRecord{
		id: "PAD-001", title: "First filing", poster: "Ada Vellér",
		objective: "Hand over the sealed arrival filing at the Exchange counter.",
		pay:       firstContractPay,
	}
}

// newContracts locates the arrivals terminal and a public Registrar counter
// in the layout. Their interaction points turn and stand with their models.
func newContracts(k *world.Kit, placed []world.Placement) (*contracts, error) {
	point := func(piece string, near rl.Vector3, front float32) (rl.Vector3, error) {
		best := float32(80)
		var at rl.Vector3
		found := false
		for _, p := range placed {
			if p.Piece != piece {
				continue
			}
			origin := rl.Vector3{X: p.At[0], Y: p.At[1] + standOn(k, p), Z: p.At[2]}
			d := rl.Vector2Distance(rl.Vector2{X: origin.X, Y: origin.Z}, rl.Vector2{X: near.X, Y: near.Z})
			if d >= best {
				continue
			}
			best, found = d, true
			turn := yawTurn(float32(p.Turns) * math.Pi / 2)
			at = rl.Vector3Add(origin, rl.Vector3RotateByQuaternion(rl.Vector3{Z: front}, turn))
		}
		if !found {
			return at, fmt.Errorf("first contract: no %s near %v", piece, near)
		}
		return at, nil
	}
	offer, err := point("terminal_kiosk", arrival, .95)
	if err != nil {
		return nil, err
	}
	delivery, err := point("registrar_counter", rl.Vector3{Z: -4}, .95)
	if err != nil {
		return nil, err
	}
	return &contracts{debt: passageDebt, offer: offer, delivery: delivery}, nil
}

func (c *contracts) accept() bool {
	if c.state != contractAvailable || c.ongoing != nil {
		return false
	}
	c.state = contractAccepted
	order := firstWorkOrder()
	c.ongoing = &order
	return true
}

func (c *contracts) deliver(feet rl.Vector3) bool {
	if c.state != contractAccepted || c.ongoing == nil || rl.Vector3Distance(feet, c.delivery) > useReach {
		return false
	}
	c.state = contractDelivered
	receipt := *c.ongoing
	receipt.debtCredit = min(c.debt, receipt.pay)
	c.debt -= receipt.debtCredit
	c.completed = append(c.completed, receipt)
	c.ongoing = nil
	return true
}

func (c *contracts) target() (rl.Vector2, bool) {
	switch c.state {
	case contractAvailable:
		return rl.Vector2{X: c.offer.X, Y: c.offer.Z}, true
	case contractAccepted:
		return rl.Vector2{X: c.delivery.X, Y: c.delivery.Z}, true
	}
	return rl.Vector2{}, false
}

func (c *contracts) prompt(feet rl.Vector3) string {
	switch {
	case c.state == contractAvailable && rl.Vector3Distance(feet, c.offer) <= useReach:
		return "Review Ada's contract"
	case c.state == contractAccepted && rl.Vector3Distance(feet, c.delivery) <= useReach:
		return "Hand over the sealed filing"
	}
	return ""
}

// contractThings consumes E before generic uses or character actions. Menus,
// vehicles, seats and traversal keep their usual control of the player.
func contractThings(
	players *illusion.Query4Where[character.Intent, transform.Transform, physics.CharacterController, character.Traversal, illusion.With[character.Player]],
	cars *illusion.Query2[vehicle.Drivable, transform.Transform],
	jobs *illusion.Res[contracts],
	m *illusion.Res[menu],
	controls *illusion.Res[character.Controls],
	driving *illusion.Res[vehicle.Driving],
	used *illusion.Res[uses],
	settings *illusion.Res[physics.Settings],
) {
	c, ok := jobs.TryGet()
	if !ok || m.Get().screen() != playing || !controls.Get().Enabled || driving.Get().Active() || used.Get().sitting != nil {
		return
	}
	_, in, tr, cc, tv, ok := players.Single()
	if !ok || in.Act != character.Interact || !cc.Grounded || tv.Mode != character.Idle {
		return
	}
	feet := rl.Vector3Subtract(tr.Translation, rl.Vector3{Y: cc.Height / 2})
	if byVehicle(cars, feet) || c.prompt(feet) == "" {
		return
	}
	if c.state == contractAvailable {
		m.Get().open(contractOffer)
	} else if c.deliver(feet) {
		m.Get().open(contractJournal)
		m.Get().stampOn("SETTLED", ledgerCredit, contractJournal)
	}
	*in = character.Intent{}
	controls.Get().Enabled = false
	settings.Get().Paused = true
}

// The menu only requests acceptance. This owns the record, after keyboard
// or mouse selection and before controls are unlocked for play again.
func contractChoice(m *illusion.Res[menu], jobs *illusion.Res[contracts]) {
	mu := m.Get()
	if c, ok := jobs.TryGet(); ok {
		if mu.acceptContract {
			c.accept()
		}
		if mu.screen() == contractJournal {
			mu.top().receipt = min(max(0, mu.top().receipt), max(0, len(c.completed)-ledgerHistoryRows))
		}
	}
	mu.acceptContract = false
}

// The offer shares the HUD's E prompt, after vehicles and before furniture.
func offerContract(
	players *illusion.Query3Where[transform.Transform, physics.CharacterController, character.Traversal, illusion.With[character.Player]],
	cars *illusion.Query2[vehicle.Drivable, transform.Transform],
	jobs *illusion.Res[contracts],
	m *illusion.Res[menu],
	prompt *illusion.Res[vehicle.Prompt],
	driving *illusion.Res[vehicle.Driving],
	used *illusion.Res[uses],
) {
	c, ok := jobs.TryGet()
	if !ok || m.Get().screen() != playing || driving.Get().Active() || used.Get().sitting != nil || prompt.Get().Key != "" {
		return
	}
	_, tr, cc, tv, ok := players.Single()
	if !ok || !cc.Grounded || tv.Mode != character.Idle {
		return
	}
	feet := rl.Vector3Subtract(tr.Translation, rl.Vector3{Y: cc.Height / 2})
	if byVehicle(cars, feet) {
		return
	}
	if text := c.prompt(feet); text != "" {
		prompt.Get().Key, prompt.Get().Text = "E", text
	}
}
