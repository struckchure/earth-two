# Personal system contracts

## The ownership rule

A system-published contract is issued to **one player**. Only that player can
accept it, deliver against it or receive its reward. Ownership is set when the
offer is issued and does not change when someone reads the terminal first.
There is no race to claim another player's system offer.

The pool contains reusable **templates**, not a set of shared unclaimed jobs.
Two players can both receive First filing: each gets their own arrival filing,
contract reference, deadline, state and debt credit. Finishing one does not
consume the other's opportunity. A common title does not mean a common job.

| Player | Template | Issued reference (illustrative) | Work allocated |
| --- | --- | --- | --- |
| Arrival A | First filing | PAD-001-A | A's arrival filing and debt credit |
| Arrival B | First filing | PAD-001-B | B's arrival filing and debt credit |

A can take PAD-001-A; B can take PAD-001-B. Both jobs can exist concurrently,
with separate receipts and outcomes.

System offers cannot be sold or reassigned to another player. Closing or
defaulting an instance does not reopen that instance for somebody else. Any
later assignment gets a new instance and a fresh allocation of work.
Player-posted contracts remain a separate public-market feature; their
posting, transfer and collection rules are described in the wider world docs.

## Pools for every tier

Maintain candidate pools for all six licence tiers. This first catalogue has
at least three repeatable templates at each tier, plus introductory and story
contracts. Pool entries are filtered by licence, issuer relations, story
state and the availability of the actual work.

Each player receives a small personal rotation of eligible offers. Completion,
declining and expiry replenish that rotation with valid assignments. Higher
licences retain access to eligible lower-tier work, including a zero-bond
day-labour option for a player who cannot afford a filed bond.

Ordinary work remains available when a story gate is closed or a faction is
hostile. Neutral work and lower-tier fallbacks keep progression from becoming
an empty terminal. Employers need funded work reserves: never issue an offer
that cannot pay, has no deliverable target or requires equipment the player
cannot obtain. A shortage in the ordinary-work reserve is a generation fault
to resolve, not a reason to duplicate somebody else's assignment.

Only one contract can be ongoing for a player at a time. Other personal
offers may remain notified while that slot is occupied, but cannot be
accepted until it is free. No offer takes a bond or starts its work deadline
merely because the player received a notification.

## Issuing a real assignment

An instance records the template, owner, unique contract reference, issuer,
tier, allocated target or cargo, complete terms, eligibility, offer expiry,
acceptance time, deadline and outcome. Template codes such as T1-02 are not
instance references. The prototype's PAD-001 is a fixed local reference;
multiplayer instances need their own unique references.

The server must check the authenticated owner on every read of private terms
and every acceptance, work submission and payout. Acceptance also checks
current eligibility, the active slot, funds and offer validity, atomically.
Delivery releases the recorded payment and any returned bond exactly once.
An attempted replay cannot create a second receipt or a second credit.

Finite shared-world resources need their own allocation. A shared warehouse
can host many jobs, but each uses distinct tagged crates. A salvage wreck can
offer separate compartments or lots, not promise the same core to two people.
A collection case reserves its debt claim for its assigned collector. If a
world event invalidates an offered target, replace the offer; if an accepted
target becomes impossible, resolve or dispute it rather than punishing the
player for another assignment's completion.

The world stays shared. Routes, prices, other players and NPCs can affect a
personal assignment. Personal ownership does not create private copies of the
whole settlement, and the issuer cannot pay twice for the same underlying
world obligation.

## Notifications, terms and acceptance

The money-card badge counts only the player's own available system offers.
The journal contains compact terminal/location/count notifications. Contract
names, objectives, pay, bonds, deadlines and penalties are reviewed at the
terminal, where the player also accepts. Viewing a notification commits them
to nothing.

At acceptance, the terms become the player's ongoing record. The journal can
then show that record and, on completion, its receipt. Other players may read
public filed records where appropriate; public visibility never gives them
the right to take over the assignment.

## Repeats and career work

Repeatable templates change concrete cargo, fault, route, case or filing
references. They do not repeatedly pay for a repair already completed or a
paper already delivered. First filing is once per arrival career; the final
charter is once per player per season. Story gates are tracked separately
from the ordinary pools so repeat labour cannot replay a career decision.

If a career changes hands in a slot sale or walk-out, settle or close its
active personal order under the existing terms, close unsigned offers and
issue fresh instances for the new player. Old receipts remain historical
records with the original taker, not a second chance to earn the payment.

## Status of this document

These are design requirements for the shared contract service. The current
game has one local First filing record, its notification, acceptance,
delivery and receipt. It does not yet implement tier-pool replenishment,
multiplayer owner validation or shared target reservations.
