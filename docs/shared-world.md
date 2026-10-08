# Shared world

Earth Two is one persistent, multiplayer world. Every player lives in the same
world, and it keeps running (prices, contracts, debts) while they're offline.

## Player identity

A player owns an Ed25519 signing key. Their account ID comes from its public
key, independently of their character's name, email, device or connection.
Account changes require that key's signature. Email is optional private contact
metadata and cannot authorize actions or recover an account.

Accounts have an editable display name. Names can be shared by multiple players;
the name is a label, and the signing key remains the owner identifier.

Players can export a passphrase-encrypted key backup and import it on desktop
or web through the game's Identity menu. Importing preserves account ownership.
Losing every key copy means losing access; email does not bypass key ownership.

## Arriving

- Each player arrives on their own drifter (a cargo hauler from Earth), at their own time.
- Every player starts as a Late Arrival, owing their passage to **Ada Vellér**.
  See [Factions](factions.md).
- A player picks their character (the man or the woman) and name once, at the
  start. Changing either means starting a new game. Clothes can still change in
  the wardrobe.
- Players start on foot. Haulers and bikes must be earned, either bought
  outright or given as a gift (a contract reward or a faction's favour).

## Time and seasons

- The game runs on a real clock: one in-game day is one real day.
- The world runs in **three-month seasons**.
- Every player plays the same story on their own clock. It starts the day they
  step off the drifter and stops when the season ends, when the Receivership
  arrives. A late joiner just has less time. See [Story](story.md).
- At the end of a season every filing is tallied, the winning settlement shapes
  the next season's world, and the world resets.

## Winning a season

A share of the season's whole economy is paid out to the **richest player at
the end of the season**, whatever their licence tier. "Richest" means net
worth: cash, property and debts owed to them, minus what they owe. The money
has to go somewhere when the world resets, and this is where it goes.

## Population

- The world has a fixed total population cap of **4,200 residents**, counting
  players and NPCs together. Offline players still occupy their slots.
- The player-to-NPC ratio is still to be decided; it determines how many of
  those 4,200 places are player slots.
- Once it's full, no new players can join: the drifters stop selling passage.
- After that, the only ways in are buying a player's slot or taking one a
  player has left for good.

## Selling a slot

Once the world is full, a place in it is the scarcest thing on the Red. A player
can sell their slot to a newcomer, either by auction on the Exchange or by
direct sale.

The buyer inherits everything the character has: money, property, standing,
debts and enemies, for better or worse. The only things they choose fresh are
the avatar's name and look. In the story, the newcomer steps off a drifter and
takes over the seller's papers, and the Exchange files it as a **Transfer**.
Buyers should read the paperwork first.

## Walking out

A player can leave the game permanently. Their character walks out past the
dome line without a mask and isn't seen again, and the slot opens for the next
person waiting to join.

Their property goes up for auction on the Exchange. Whoever takes the open slot
inherits whatever hasn't sold by then, along with the character's standing and
debts, and picks a new name and look.

## Death

Vehicle impacts affect both NPCs and players. At **10 km/h closing speed** a
character becomes critical and cannot act; at **30 km/h** they die immediately.
Both states leave a fallen body that a vehicle can run over. Injured NPCs do
not become healthy again when the player leaves the area and returns.

In the current prototype, an incapacitated player can press R for emergency
recovery or respawn at the Pads. The points and debt flow below is still to
be implemented.

Players can hurt and kill each other. Fights can be lethal, and weapons are
anything money can buy (see [Economy](economy.md)).

A player who dies pays points to respawn wherever they choose. A player without
enough points has the respawn paid for them and owes the cost to whoever
covered it: Ada, an NPC, a faction or another player. That creditor can sell
the debt or file a collection on it, like any other debt. See
[The Exchange](exchange.md).

## Personal system work

The world is shared, but a system-issued contract belongs to one player.
Every licence tier has a pool of templates from which personal instances are
issued. Two arrivals can receive the same kind of courier job with different
filings, references and payments; neither can take or finish the other's job.
One completion does not empty the other player's terminal.

Shared cargo, fault tickets, claims and debt cases are allocated to specific
assignments so the issuer never promises the same recoverable item or settled
obligation twice. Routes, prices, danger and interference remain shared.
See [the system contract rules](contracts/system-contracts.md).

## Players and each other

- Player-posted contracts can be offered to other players, so much of the
  public contract board is player-made. Personal system offers stay with
  their assigned player.
- Collection work on a defaulted debt can be assigned to NPCs or to a player.
  System-published cases are personal assignments with reserved debt claims,
  rather than one system contract several users can take.
- Players can buy and sell each other's debts.
- The Late Arrivals could become a real, player-run faction.
