# Real estate, construction and town planning

Date: 2026-10-11

Status: Draft proposal for discussion; no implementation authorized by this document.

## Goal

Make land and buildings a playable part of Earth Two's economy. Players should
be able to buy and sell property, rent space, build for themselves, hire someone
to build, and earn a living as builders or property developers. Town planning
must give that activity a coherent physical shape: usable streets, connected
services, distinct neighbourhoods and room for settlements to grow.

The central loop is: find an opportunity → secure property rights → plan a
project → source materials and labour → build and inspect → occupy, rent or
sell → maintain and improve.

## Existing foundations

- [Economy](../docs/economy.md) already makes property and land claims tradable,
  with district pricing, scarce dome space and property contributing to wealth.
- [The Exchange](../docs/exchange.md) records and enforces contracts; it does not
  make laws. Planning should operate through filed agreements between claim
  holders, factions and service providers, rather than introduce a city government.
- [The settlement](../docs/settlement.md) establishes dense, vertical Landfall,
  Charter Row, the Pads and the more dispersed Fringe settlements.
- [Shared world](../docs/shared-world.md) establishes persistent ownership,
  offline economic activity, a 4,200-resident cap, slot transfers, walk-out
  auctions and three-month seasons. More housing does not increase that cap.
- Existing licence rights include stall rental at Contractor and warehouse
  holding, contract posting and NPC crew hiring at Agent. New property rights
  must fit those gates; their detailed mapping is still a design decision.

Everything below is proposed. Prices, durations, capacities and progression
requirements need balancing before they become settled world rules.

## 1. Define what can be owned

Separate the land claim, the building and the usable space inside it.

| Record | Purpose | Examples |
| --- | --- | --- |
| Parcel | Bounded land with a claim holder, permitted uses and access rights | Fringe plot, Pads yard |
| Building | A structure attached to a parcel or an existing parent structure | House, workshop, warehouse, Hull conversion |
| Unit | Independently occupiable space inside a building | Room, apartment, stall, office |
| Right or obligation | What ownership permits and what it owes | Lease, utility connection, passage easement, lien |

Landfall needs volumetric boundaries: stacked rooms and deck spaces cannot be
represented only by flat plots. Units share entrances, corridors and building
services. Buying a unit must not imply buying its whole building or the land.

Every sale should show the exact asset, seller, asking price, condition,
permitted uses, access, utility capacity, occupants, recurring charges and
filed obligations. Distinguish a disputed claim from a clean recorded title.
Transfers must preserve applicable leases, access rights and liabilities.

Start with one recorded owner per asset, including NPC or faction owners.
Defer joint ownership and separately traded shares until the basic registry works.

## 2. Plan the town before releasing plots

Create an authored planning layer over the existing world. Identify existing
structures and ownership before opening land for development, and reserve
space for streets, utilities, contract destinations and future expansion.

| Area | Proposed development character | Planning priorities |
| --- | --- | --- |
| Hull / Stacks | Rooms, stalls, small workshops, internal conversions | Shared corridors, vertical access, structure and life support |
| Charter Row | Higher-value homes, offices and controlled infill | Existing character, frontage, service capacity and faction agreements |
| Pads | Warehouses, freight yards, workshops and worker accommodation | Hauler access, loading areas and separation from active landing operations |
| Fringers' hold | Farm plots, greenhouses, low-rise homes and salvage workshops | Water, power, agricultural access and expansion space |
| Quiet Book's haven | Informal occupation and off-book construction | Contested rights and access; later gameplay scope |

Each district plan should record:

- Parcel and unit boundaries, stable IDs and existing claims.
- Allowed uses: residential, commercial, industrial, agricultural, mixed use,
  infrastructure or protected space.
- Buildable envelope: footprint, height or deck limits, setbacks, entrances,
  structural limits and protected traversal routes.
- Street hierarchy: inter-seat roads, local streets, service lanes, pedestrian
  routes, ladders and shared vertical circulation.
- Service corridors and available water, power, waste and enclosed-air capacity.
- Civic and shared space, freight access and land reserved for later growth.

Use these rules to preserve the game's movement and contract routes. No build
may seal a required doorway, trap a resident, block the South gate, occupy a
spawn point or cut the only route to a job destination. Reserve these spaces
explicitly rather than discovering conflicts after players buy them.

Planning agreements belong to the relevant landholders and service providers;
Registrars record them and arbitrate filed disputes. Initial rules are authored.
Later, players can propose subdivisions, parcel mergers, use changes and
infrastructure extensions through multi-party contracts. Ownership alone does
not permit unilateral changes to shared streets or neighbouring rights.

Deliverable before implementation: one annotated pilot district map with
saleable properties, buildable parcels, protected routes and service budgets.

## 3. Buying, selling and renting

The first market should support fixed-price listings, inspection and purchase.
Add negotiated offers, auctions and brokered deals once transfers are reliable.
Use Marks for the initial filed property market.

Purchase flow:

1. Browse Exchange listings or inspect the property in the world.
2. Review title, boundaries, condition, access, occupancy and obligations.
3. Submit a purchase against the current listing price and version.
4. The server verifies authority, eligibility, funds and transfer restrictions.
5. Payment, title transfer, listing closure and the filing commit together.
6. Both parties receive a receipt; access permissions update for all clients.

Only one buyer can succeed. Retried requests must not charge twice. Sellers
cannot remove disclosed obligations by relisting, and protected communal or
story-critical assets cannot enter the ordinary market.

Rentals add deposits, rent schedules, permitted use, duration, renewal,
maintenance responsibilities and default terms. Selling a tenanted property
preserves its lease. Any repossession or eviction needs notice and a defined
dispute process, plus safe handling of occupants and stored belongings.

Later scope: mortgages, secured development loans, foreclosures, brokers,
portfolio management and property auctions. These depend on the debt and
enforcement systems and should not block the first sale.

## 4. Building as gameplay

Begin with approved modular blueprints that use the existing world kit.
Players choose a compatible design, position it within the allowed envelope,
and configure a limited set of rooms, entrances and finishes. Broader modular
design can follow once collision, navigation and performance are dependable.

Construction lifecycle:

`Draft → Validated → Funded → Under construction → Inspection → Occupied`

Projects may also pause, fail inspection or be cancelled under recorded terms.
Before accepting funding, validate ownership or build permission, planning
agreements, geometry, route access, service capacity and the material estimate.
Reserve the parcel's build envelope and service allocation so simultaneous
projects cannot spend the same capacity.

Work progresses through visible stages: site preparation, foundations or
structural alterations, shell, services and fit-out. Each stage consumes
materials and verified labour. Time alone should not create a finished building.
Owners can do eligible work themselves or hire players and NPC crews.

The project screen shows the quote, delivered stock, remaining work, assigned
workers, blocked requirements, expected completion and payments. Offline work
uses server time and assigned crews; offline owners do not receive free labour.

Completion checks confirm the agreed blueprint, entrances, services and usable
space before occupancy is enabled. Renovation, extension and demolition use
the same permission checks. Cancellation specifies refunds, consumed material,
salvage, unpaid work and how an unfinished site is made safe.

## 5. Construction and property careers

Construction should sustain a career without requiring the worker to own land.

| Role | Work | Income |
| --- | --- | --- |
| Labourer | Preparation, deliveries, assembly and repairs | Wages and work orders |
| Builder / contractor | Quote and complete construction stages | Labour fees and project margin |
| Specialist | Install services, fit interiors or inspect completed work | Specialist contracts |
| Developer | Secure a site, finance a project and organize delivery | Sale proceeds or rent |
| Property manager | Maintain buildings, manage leases and arrange repairs | Management fees |
| Broker | Bring parties together and arrange complex property deals | Disclosed commissions |

These are occupations, not replacements for Exchange licence tiers. Offer
sponsored day labour to Unlisted arrivals, filed labour to Hands and larger
responsibilities as existing licence rights permit. Confirm the exact gates
before implementation, especially self-building and posting construction jobs.

Construction contracts specify scope, materials, milestones, acceptance,
deadline, bond, payment, change orders and default handling. Funded milestone
escrow protects completed work and prevents an owner from commissioning work
without money. Disputes follow Exchange arbitration.

NPC builders keep the market usable when players are unavailable, but have
finite crews, inventory and schedules. Persistent worker NPCs count toward the
existing population cap. Reputation should reflect delivery, quality and
disputes. System-issued construction jobs remain personal assignments; shared
project work must reserve distinct tasks so it cannot be paid out twice.

## 6. Connect property to the wider economy

- Source salvage, structural components, machinery and services through trade
  and hauling; construction creates demand for existing professions.
- Let location, access, condition, services and productive use affect value.
- Make workshops, shops, storage and farms useful through their corresponding
  gameplay systems. Ownership alone should not generate unlimited passive money.
- Introduce disclosed maintenance and service costs with manageable grace
  periods; decide any vacancy or holding charges through gameplay balancing.
- Maintain accessible starter accommodation and phased plot releases so early
  buyers cannot deny all newcomers a place to live or work.
- Value property for season scoring using a defined, manipulation-resistant
  method; asking prices and circular sales cannot establish net worth. Deduct
  secured debt and avoid counting land, building and units twice.
- Respect slot transfers and walk-out auctions. Define how leases, active builds,
  escrow and contractor obligations move with an estate before enabling them.

Season reset and carryover remain open in the existing world design. The
feature needs explicit settlement rules for titles, deposits, outstanding
work, loans and unfinished buildings; do not imply permanent cross-season ownership.

## 7. Player-facing tools

- **Property map:** parcels, ownership, permitted uses, listings, access and
  service capacity, with clear treatment of overlapping floors in Landfall.
- **Property record:** title, occupants, obligations, condition, costs and history.
- **Build preview:** blueprint placement, material estimate and specific reasons
  a proposal fails validation before the player commits funds.
- **Project board:** quotes, jobs, deliveries, milestones, payments and disputes.
- **Portfolio:** owned and rented space, income, bills, maintenance and active builds.

Present these through the Exchange's existing paperwork language and in-world
interaction, with matching desktop and browser behavior.

## 8. Technical approach and dependencies

Treat this as shared world simulation. The README notes that current movement
and the first contract remain local; authoritative economic and world-state
integration is a prerequisite, not an already completed foundation.

Candidate records: `DistrictPlan`, `Parcel`, `Property`, `Unit`, `Title`,
`AccessRight`, `UtilityAllocation`, `Listing`, `Lease`, `Blueprint`,
`ConstructionProject`, `WorkOrder`, `MaterialDelivery`, `Inspection` and
`PropertyLedgerEntry`. Keep stable asset IDs separate from their visual pieces.

Use the server module for ownership, permissions, money, escrow, materials,
project transitions and an auditable transaction history. Bind actions to the
existing account identity and use atomic, idempotent commands with version checks.
Clients preview proposals; the server validates all consequential changes.

Extend `world/` and the authored layouts to associate structures with parcels,
units and persistent projects. Reuse `tools/world/` for blueprint assets and
`game/` for inspection, map overlays and build interaction. Dynamic structures
need consistent rendering, collision, navigation and access state for nearby
players, reconnecting players and both client platforms.

Set geometry and active-site budgets before broad free-form building. Prefer
staged prefab changes initially. Validate expensive spatial proposals outside
the render loop, with final authority and conflict checks at commit time.

## 9. Delivery phases

| Phase | Scope | Exit condition |
| --- | --- | --- |
| 0. Planning foundation | Pilot district map, property boundaries, title rules, protected routes, service budgets and licence mapping | Every pilot asset has an unambiguous owner, permitted use and reachable entrance |
| 1. Property market | Authoritative funds/title foundation; inspect, list, buy and resell existing properties | Two players trade a property persistently without duplicate payment or ownership |
| 2. Build for yourself | One parcel type, one small blueprint, materials, stages, inspection and persistence | A player builds and occupies a property that other clients see correctly |
| 3. Build for a living | Quotes, funded work orders, NPC/player labour, milestone payments and disputes | A builder earns from another player's project without owning the parcel |
| 4. Property operations | Leases, maintenance, repairs, management and useful commercial space | A tenanted property can operate and change owners while preserving obligations |
| 5. Settlement growth | More districts, utilities, subdivisions, extensions, auctions and finance | Development expands the settlement without breaking access, capacity or economic rules |

Recommended first slice: a small authored cluster of existing properties and
one serviced buildable plot in Landfall, chosen after the map audit. Prove one
complete purchase → build → hire → occupy → resell loop before broad expansion.

## 10. Acceptance checks for implementation

- Simultaneous purchases, stale listings and retried commands cannot duplicate
  title, funds or payouts; unauthorized accounts cannot modify property.
- Reconnects and server restarts preserve titles, escrow, deliveries and work.
- Vertical units cannot overlap, sell their common corridors or spend the same
  structural or utility allowance twice.
- Invalid builds explain the blocked rule and cannot obstruct protected routes,
  strand occupants or collide with another reserved project.
- Materials and labour cannot be reused across projects; inspection and payment
  are tied to the agreed milestone and cannot be replayed.
- Cancellation, default, sale during construction and walk-out transfers settle
  obligations consistently. Initially block unsupported transfer states explicitly.
- Completed buildings have usable collision, entrances and navigation, and render
  within agreed budgets on desktop and browser.
- Season valuation and settlement handle unit ownership, debt and escrow without
  double counting or rewarding self-dealing.

## Decisions to resolve before building

1. Which pilot properties and plot are available, and who initially owns them?
2. What property and self-build rights belong to each licence tier?
3. How much design freedom fits the first blueprint editor?
4. What prices, labour durations and material costs suit a three-month season?
5. Which utilities are simulated initially, and who sells their capacity?
6. How do rent default, abandoned sites and disputed claims become playable cases?
7. What property valuation and end-of-season settlement rules apply?
8. When should players gain the ability to negotiate district planning changes?

Resolve the pilot map, ownership rules and licence mapping first. They determine
what can be sold, where construction can occur and which careers are available.
