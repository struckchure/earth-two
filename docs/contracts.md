# Contract types

Contracts are built from a small set of templates, each filled in by the world:
who posted it, what goods, where and by when. The "Plays with" column names the
mechanics already in the game (traversal, interact, pick up, punch, wardrobe),
so the first templates can ship without new systems. For how filing, bonds and
defaults work, see [The Exchange](exchange.md).

System-published contracts are **personal instances**: one assigned player,
one taker and one settlement. Templates are pooled for every licence tier;
the same template can produce distinct jobs for different players. Another
player cannot claim, consume or receive payment for your system contract.
Player-posted jobs use their separate public-market rules.

The [contract review book](contracts/README.md) contains the first tier-by-tier
writing pass, with issuer messages and proposed terms. Its [system rules](contracts/system-contracts.md)
define ownership and replenishment, and its [voice guide](contracts/tone.md)
sets the tone. These drafts are not a claim that the full pools already run
in the prototype.

| Contract | Filed or off-book | Example | Plays with | From tier |
| --- | --- | --- | --- | --- |
| Courier | Filed | Carry a sealed filing from the Exchange to Charter Row before the session closes | Traversal, time limit | 1 |
| Haul | Filed | Move 20 crates from the Pads to a Hull warehouse | Pick up, routing | 1 |
| Repair run | Filed | Replace a valve deep in the Hull's lower decks | Climbing, ladders, interact | 1 |
| Buy order | Filed | Find 5 power cells under 40 marks each | Trading, arbitrage | 1 |
| Escort | Filed | Get a Fringer caravan to the dome gate | Pathing, combat | 2 |
| Salvage claim | Filed | Strip a wrecked terraformer before a rival crew does | Traversal, pick up | 2 |
| Collection | Filed | Recover a defaulted debt: the money, the goods, or the person | Chase, persuasion, combat (can turn lethal) | 4 |
| Smuggle | Off-book | Get unfiled medicine past the dome gate checks | Stealth routes, wardrobe disguise | 0 |
| Theft | Off-book | Lift a ledger from a Charter Family office | Climbing, vaulting, interact | 1 |
| Forgery drop | Off-book | Slip a forged seal into the Exchange records | Interact, timing | 2 |
| Sabotage | Off-book | Make a rival's warehouse lose power for a night | Traversal, interact | 2 |
| Run a rumour | Either | Spread (or bury) news that moves scrip prices | Talking to NPCs | 1 |

## First work order: First filing

The first playable job is **sponsored day labour**, available to an Unlisted
Late Arrival. It teaches accepting terms and making a delivery before the
player qualifies for filed courier contracts. The Hand requirement for filed
couriers follows [the Exchange's licence table](exchange.md#licence-tiers-are-the-games-ranks).

- **Poster:** Ada Vellér, who owns the player's 2,000-mark passage debt.
- **Offer:** the arrivals terminal at the Pads, near the *Patience* caravan stop.
- **Work:** collect a sealed arrival filing on acceptance, carry it to Landfall,
  and hand it over at a public Registrar counter on the Exchange floor.
- **Pay:** 150 marks credited directly against Ada's passage debt, leaving
  1,850 marks. It is paid once, on handover.
- **Bond and licence:** no bond; Unlisted day labour needs no Hand licence.
- **Deadline and penalty:** this introductory order is untimed, with no default
  penalty. Later filed contracts use the normal bond and default rules.

The player reviews the terms with E at the terminal and explicitly chooses
**Accept contract** or **Leave it for now**. Esc also leaves the order available.
Acceptance issues the filing and marks the Exchange delivery point; reaching
it alone does not finish the work. E at the counter hands it over and shows
the receipt. J opens the contract journal in each state: one **Ongoing** slot
and a separate **Completed** history. Delivery clears the ongoing slot and
adds the accepted terms and debt credit to completed receipts. Empty sections
say so; unaccepted offers appear separately under **Available contracts**.

When offers are available, the compact money card carries a numeric badge.
Its count and the journal's offer list come from the same available work
orders. The journal carries compact notifications showing which terminal and
location have offers, and how many. Players visit the terminal to view the
contract's issuer, objective, pay and terms, and to accept it. Open the book
with J or by clicking the money card or its badge. Reading a notification
leaves the offer available; accepting at the arrivals terminal removes it
from the badge and moves it into ongoing work. Delivered work appears only
in completed receipts.

J presents a personal accounting book. Its summary shows spendable **Balance**
and outstanding **Debt** in marks. The ruled **Debts owed** table identifies
Ada and her passage debt, with columns for original amount, repaid and due,
followed by totals. **Completed contracts** lists filing references, contract
names and payers, actual debt credits and delivery status, with a total for all
receipts. Four receipts fit on the page; scroll to browse older entries.
Available-contract notifications and the ongoing work order remain on the
facing page.
A compact balance/debt summary in play matches the time/weather card's
footprint; J opens the full breakdown and ongoing/completed work. Arrivals
have zero spendable marks and owe
2,000; completing First filing leaves the balance at zero and debt at 1,850.
The payment is a debt credit, not an additional cash reward. Completed records
are shown newest first, with scrolling to browse the history.

This work order does not grant Standing, Quiet or a licence, and does not file
an enforceable contract in the player's name. The Exchange receives the arrival
filing; the player is still Unlisted and in debt. It does not grant a vehicle.
The opening remains on foot, as specified in [Shared world](shared-world.md).

The current playable prototype keeps the order and debt in memory for the
running game. Persistent per-player records, offline deadlines, bonds and
collection contracts remain part of the shared-world design, not implemented
by this first work order.

## Contracts aren't one-offs

A theft can lead to a filed collection contract on the thief, which can be
bought and sold, and which might be on you. Choices leave paper behind, and the
paper comes back.

## Players post contracts too

From Agent (tier 3), a player can post contracts. Hiring NPCs to run hauls,
guard stalls or collect debts is the mid-game: the player stops doing the work
and starts running a business. Other players can take any contract a player
posts, so much of the board is player-made.
