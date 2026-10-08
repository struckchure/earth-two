package game

import (
	"testing"
	"time"

	rl "github.com/gen2brain/raylib-go/raylib"
	"github.com/mlange-42/ark/ecs"
	"github.com/struckchure/earth-two/character"
	"github.com/struckchure/earth-two/vehicle"
	"github.com/struckchure/earth-two/world"
	"github.com/struckchure/illusion"
	"github.com/struckchure/illusion/input"
	"github.com/struckchure/illusion/physics"
	"github.com/struckchure/illusion/transform"
	"github.com/struckchure/illusion/window"
)

func firstContract(t *testing.T) *contracts {
	t.Helper()
	k, err := world.Load("../assets", "world/world.json")
	if err != nil {
		t.Fatal(err)
	}
	placed, err := world.Layout("../assets", "world/landfall.json")
	if err != nil {
		t.Fatal(err)
	}
	c, err := newContracts(k, placed)
	if err != nil {
		t.Fatal(err)
	}
	return c
}

// Exercise the real E handler, menu keyboard/mouse, record and control lock,
// without opening a graphical window or loading character meshes.
func contractRig(t *testing.T) (*illusion.App, *contracts, ecs.Entity, *input.Keys) {
	t.Helper()
	app := illusion.New()
	c := firstContract(t)
	keys := input.NewButtonInput[input.Key]()
	app.InsertResource(illusion.R(c), illusion.R(&menu{}), illusion.R(keys),
		illusion.R(input.NewButtonInput[input.MouseButton]()), illusion.R(&input.Mouse{}),
		illusion.R(&character.Controls{Enabled: true}), illusion.R(menuWardrobe()),
		illusion.R(&window.Window{Width: 1280, Height: 800, Scale: 1}),
		illusion.R(&illusion.AppExit{}), illusion.R(&vehicle.Driving{}),
		illusion.R(&vehicle.Prompt{}), illusion.R(&uses{}), illusion.R(&physics.Settings{}))
	p := ecs.NewMap5[character.Player, character.Intent, transform.Transform, physics.CharacterController, character.Traversal](app.World).
		NewEntity(&character.Player{}, &character.Intent{},
			ptrTransform(c.offer), &physics.CharacterController{Height: 1.8, Grounded: true}, &character.Traversal{})
	ecs.NewMap[character.Outfit](app.World).Add(p, &character.Outfit{})
	app.AddSystems(illusion.Update, illusion.Chain(illusion.Fn8(contractThings), illusion.Fn8(menuInput), illusion.Fn2(contractChoice), illusion.Fn4(lockControls)))
	t.Cleanup(app.Cleanup)
	return app, c, p, keys
}

func ptrTransform(feet rl.Vector3) *transform.Transform {
	t := transform.FromTranslation(rl.Vector3Add(feet, rl.Vector3{Y: .9}))
	return &t
}

func TestFirstContractReviewDeclineAndAccept(t *testing.T) {
	app, c, p, keys := contractRig(t)
	if len(c.availableContracts()) != 1 {
		t.Fatal("the arrival must see one available offer")
	}
	m := ecs.GetResource[menu](app.World)
	in := ecs.NewMap[character.Intent](app.World).Get(p)
	tick := func() { app.Tick(time.Second / 60) }
	tap := func(k input.Key) { keys.Press(k); tick(); keys.Release(k); keys.Clear(); tick() }

	*in = character.Intent{Act: character.Interact, Move: rl.Vector3{X: 1}, Jump: true}
	tick()
	if m.screen() != contractOffer || c.state != contractAvailable || c.debt != passageDebt {
		t.Fatalf("E must review without accepting: screen %v, record %+v", m.screen(), c)
	}
	if *in != (character.Intent{}) || ecs.GetResource[character.Controls](app.World).Enabled || !ecs.GetResource[physics.Settings](app.World).Paused {
		t.Fatal("review must consume E and movement, and pause controls and physics")
	}
	tap(rl.KeyDown)
	tap(rl.KeyEnter) // Leave it for now.
	if m.screen() != playing || c.state != contractAvailable || len(c.availableContracts()) != 1 || !ecs.GetResource[character.Controls](app.World).Enabled {
		t.Fatal("declining must resume play and keep the job available")
	}
	tap(rl.KeyJ)
	tap(rl.KeyEnter) // Journal's Back, not acceptance from anywhere.
	if m.screen() != playing || c.state != contractAvailable {
		t.Fatal("the journal must not accept a job remotely")
	}
	in.Act = character.Interact
	tick()
	tap(rl.KeyEscape)
	if m.screen() != playing || c.state != contractAvailable {
		t.Fatal("Esc must leave the offer available")
	}
	in.Act = character.Interact
	tick()
	tap(rl.KeyEnter)
	if m.screen() != playing || c.state != contractAccepted || len(c.availableContracts()) != 0 || c.debt != passageDebt || m.acceptContract {
		t.Fatalf("explicit acceptance: screen %v, record %+v, pending %v", m.screen(), c, m.acceptContract)
	}
	tap(rl.KeyJ)
	tap(rl.KeyJ)
	if m.screen() != playing || c.state != contractAccepted {
		t.Fatal("the journal must reopen and close without changing the record")
	}
}

func TestFirstContractMouseAcceptanceAndDelivery(t *testing.T) {
	app, c, p, _ := contractRig(t)
	m := ecs.GetResource[menu](app.World)
	in := ecs.NewMap[character.Intent](app.World).Get(p)
	tr := ecs.NewMap[transform.Transform](app.World).Get(p)
	tick := func() { app.Tick(time.Second / 60) }
	*tr = *ptrTransform(c.delivery)
	in.Act = character.Interact
	tick()
	if c.state != contractAvailable || c.debt != passageDebt || m.screen() != playing {
		t.Fatal("delivery before acceptance must do nothing")
	}
	*tr = *ptrTransform(c.offer)
	in.Act = character.Interact
	tick()
	l := layoutFor(contractOffer, 1280, 800, uiScale(ecs.GetResource[window.Window](app.World)))
	ecs.GetResource[input.Mouse](app.World).Position = middle(l.buttons[0])
	buttons := ecs.GetResource[input.MouseButtons](app.World)
	buttons.Press(rl.MouseButtonLeft)
	tick()
	buttons.Release(rl.MouseButtonLeft)
	buttons.Clear()
	if c.state != contractAccepted || m.screen() != playing {
		t.Fatal("clicking Accept must accept and return to play")
	}
	*tr = *ptrTransform(c.delivery)
	tick()
	if c.state != contractAccepted {
		t.Fatal("arriving at the counter must not deliver automatically")
	}
	in.Act = character.Interact
	tick()
	if c.state != contractDelivered || len(c.availableContracts()) != 0 || c.debt != 1850 || m.screen() != contractJournal {
		t.Fatalf("handover must show receipt and credit Ada's debt: %+v, screen %v", c, m.screen())
	}
	if c.deliver(c.delivery) || c.accept() || c.debt != 1850 {
		t.Fatal("a delivered contract must never accept or pay twice")
	}
}

func TestContractInteractionRequiresFreeGroundedPlayer(t *testing.T) {
	for _, condition := range []string{"far away", "upstairs", "airborne", "traversing", "sitting", "driving", "by a vehicle", "paused"} {
		t.Run(condition, func(t *testing.T) {
			app, c, p, _ := contractRig(t)
			in := ecs.NewMap[character.Intent](app.World).Get(p)
			tr := ecs.NewMap[transform.Transform](app.World).Get(p)
			cc := ecs.NewMap[physics.CharacterController](app.World).Get(p)
			m := ecs.GetResource[menu](app.World)
			switch condition {
			case "far away":
				tr.Translation.X += 3
			case "upstairs":
				tr.Translation.Y += 3.6
			case "airborne":
				cc.Grounded = false
			case "traversing":
				ecs.NewMap[character.Traversal](app.World).Get(p).Mode = character.Vault
			case "sitting":
				ecs.GetResource[uses](app.World).sitting = &placedSpot{}
			case "driving":
				ecs.GetResource[vehicle.Driving](app.World).Vehicle = p
			case "by a vehicle":
				ecs.NewMap2[vehicle.Drivable, transform.Transform](app.World).NewEntity(&vehicle.Drivable{}, ptrTransform(c.offer))
			case "paused":
				m.open(paused)
			}
			in.Act = character.Interact
			app.Tick(time.Second / 60)
			if m.screen() == contractOffer || c.state != contractAvailable {
				t.Fatalf("%s must not open or accept the contract", condition)
			}
		})
	}
}

func TestFirstContractPointsAreAccessible(t *testing.T) {
	c := firstContract(t)
	if rl.Vector3Distance(c.offer, arrival) > 60 || rl.Vector3Length(c.delivery) > 10 {
		t.Fatalf("wrong district for first job: offer %v, delivery %v", c.offer, c.delivery)
	}
	for name, at := range map[string]rl.Vector3{"terminal": c.offer, "counter": c.delivery} {
		t.Run(name, func(t *testing.T) {
			probe := *c
			if name == "counter" {
				probe.accept()
			}
			l := newLandfall(t, rl.Vector3Add(at, rl.Vector3{Y: .2}))
			l.tick(60)
			if !l.cc.Grounded || rl.Vector3Distance(l.feet(), at) > .3 || probe.prompt(l.feet()) == "" {
				t.Fatalf("can't stand and interact at %s: feet %v, point %v", name, l.feet(), at)
			}
		})
	}
}

func TestContractRoutePreservesManualMarker(t *testing.T) {
	c := firstContract(t)
	m := &worldMap{dest: rl.Vector2{X: 20, Y: 30}}
	r, name := routeForContract(m, c)
	if !r.marked || name != "Arrivals terminal" || m.marked {
		t.Fatal("initial route must guide to the terminal without writing a manual mark")
	}
	c.accept()
	r, name = routeForContract(m, c)
	if name != "Exchange delivery" || r.dest != (rl.Vector2{X: c.delivery.X, Y: c.delivery.Z}) {
		t.Fatal("acceptance must guide to the delivery counter")
	}
	m.marked = true
	r, name = routeForContract(m, c)
	if name != "" || r.dest != m.dest || !r.marked {
		t.Fatal("a manual destination must take priority")
	}
	m.marked = false
	c.deliver(c.delivery)
	r, _ = routeForContract(m, c)
	if r.marked {
		t.Fatal("completion must remove the contract route")
	}
}

func TestContractArchivesReceiptWithoutPayingCash(t *testing.T) {
	c := firstContract(t)
	if c.balance != 0 || c.ongoing != nil || len(c.completed) != 0 {
		t.Fatal("an arrival must start with no cash, ongoing work or history")
	}
	c.balance = 73 // Existing spendable money is independent of Ada's credit.
	if !c.accept() || c.ongoing == nil || len(c.completed) != 0 {
		t.Fatal("acceptance must fill only the ongoing slot")
	}
	accepted := c.ongoing
	terms := *accepted
	if !c.deliver(c.delivery) || c.ongoing != nil || len(c.completed) != 1 {
		t.Fatal("delivery must free the slot and archive exactly one receipt")
	}
	r := c.completed[0]
	if r.id != terms.id || r.title != terms.title || r.poster != terms.poster || r.objective != terms.objective || r.pay != terms.pay || r.debtCredit != 150 {
		t.Fatalf("receipt must preserve accepted terms and actual credit: %+v", r)
	}
	if c.balance != 73 || c.debt != 1850 {
		t.Fatalf("debt repayment must not also pay cash: balance %d, debt %d", c.balance, c.debt)
	}
	accepted.title = "Changed after completion"
	if c.completed[0].title != terms.title {
		t.Fatal("completed receipts must be independent of the old ongoing record")
	}
	if c.deliver(c.delivery) || c.accept() || len(c.completed) != 1 || c.balance != 73 || c.debt != 1850 {
		t.Fatal("repeated input must not create duplicate history or credit")
	}
}

func TestContractCannotReplaceOccupiedSlot(t *testing.T) {
	occupied := &contractRecord{id: "EXISTING", title: "A job already in progress"}
	c := &contracts{debt: passageDebt, ongoing: occupied}
	if c.accept() || c.ongoing != occupied || c.state != contractAvailable {
		t.Fatal("an offer must not displace an ongoing contract")
	}
}

func TestJournalBrowsesHistoryWithoutChangingAccount(t *testing.T) {
	app, c, _, keys := contractRig(t)
	c.completed = []contractRecord{
		{id: "OLDER", title: "Earlier receipt", debtCredit: 100},
		{id: "SECOND", title: "Second receipt", debtCredit: 25},
		{id: "THIRD", title: "Third receipt", debtCredit: 50},
		{id: "FOURTH", title: "Fourth receipt", debtCredit: 75},
		{id: "LATEST", title: "Latest receipt", debtCredit: 150},
	}
	c.balance, c.debt = 73, 1600
	m := ecs.GetResource[menu](app.World)
	keys.Press(rl.KeyJ)
	app.Tick(time.Second / 60)
	keys.Release(rl.KeyJ)
	keys.Clear()
	if m.screen() != contractJournal {
		t.Fatal("J must open the journal")
	}
	l := layoutFor(contractJournal, 1280, 800, uiScale(ecs.GetResource[window.Window](app.World)))
	mouse := ecs.GetResource[input.Mouse](app.World)
	mouse.Position, mouse.Wheel = middle(l.rows[1]), -1
	for range 4 {
		app.Tick(time.Second / 60)
	}
	if m.top().receipt != 1 {
		t.Fatal("history browsing must stop when the oldest receipt fills the last page")
	}
	mouse.Wheel = 1
	for range 4 {
		app.Tick(time.Second / 60)
	}
	if m.top().receipt != 0 {
		t.Fatal("history browsing must return to the newest receipt")
	}
	if c.balance != 73 || c.debt != 1600 || len(c.completed) != 5 || c.completed[0].id != "OLDER" || c.completed[4].id != "LATEST" {
		t.Fatal("browsing must not pay, edit or reorder completed work")
	}
}

func TestMoneyCardAndBadgeOpenJournalWithoutAcceptingOffer(t *testing.T) {
	for _, target := range []string{"card", "badge"} {
		t.Run(target, func(t *testing.T) {
			app, c, player, keys := contractRig(t)
			*ecs.NewMap[transform.Transform](app.World).Get(player) = *ptrTransform(c.delivery)
			win := ecs.GetResource[window.Window](app.World)
			p := painter{s: uiScale(win)}
			card := accountSummaryRect(p, float32(win.Width))
			at := middle(card)
			if target == "badge" {
				badge := accountBadgeRect(p, card)
				// Click the part of the badge outside the card itself.
				at = rl.Vector2{X: badge.X + badge.Width/4, Y: badge.Y + badge.Height/4}
			}
			ecs.GetResource[input.Mouse](app.World).Position = at
			buttons := ecs.GetResource[input.MouseButtons](app.World)
			buttons.Press(rl.MouseButtonLeft)
			app.Tick(time.Second / 60)
			buttons.Release(rl.MouseButtonLeft)
			buttons.Clear()
			m := ecs.GetResource[menu](app.World)
			if m.screen() != contractJournal || c.state != contractAvailable || len(c.availableContracts()) != 1 || c.ongoing != nil || c.debt != passageDebt || c.balance != 0 {
				t.Fatal("clicking the money card or badge must only open the journal")
			}
			keys.Press(rl.KeyEnter) // Back, not remote acceptance.
			app.Tick(time.Second / 60)
			keys.Release(rl.KeyEnter)
			keys.Clear()
			if m.screen() != playing || c.state != contractAvailable || len(c.availableContracts()) != 1 {
				t.Fatal("viewing and closing available offers must not accept them")
			}
		})
	}
}
