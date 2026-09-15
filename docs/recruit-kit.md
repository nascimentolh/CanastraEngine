# Recruit kit (48-hour starter gear)

Copied from the private server's design, which works well. The data half is implemented; the runtime half
waits for inventories and mail.

**The kit belongs to the server owner.** Each owner sets it in Studio (Classes tab, a starting class,
Initial items): which items, how many, whether they are worn and how long they last. The kit listed below
is only the suggested default that migration fills in. The runtime rules apply to whatever kit the owner
sets.

## Data (done)

Each starting class (`canastra_data::class::StartingClass`) lists its `initial_items`. An item with
`lasts_minutes` disappears that many minutes after the character is created; `None` keeps it forever.

The suggested kit, migrated from the private server's `initialEquipment.xml`, has two layers per starting
class:

- **Permanent:**
  - Fighters: Squire's Sword, Shirt and Pants (worn), Soulshot No Grade ×3000.
  - Mystics: Apprentice's Wand, Tunic and Stockings (worn), Spiritshot No Grade ×3000.
  - Everyone also gets Lesser Healing Potion ×50 and Adventurer's Scroll of Escape ×5.
- **For 2880 minutes (48 hours), worn:** top no-grade gear.
  - Fighters: Hard Leather Shirt, Gaiters and Helmet, Bracer, Boots, and a weapon: Falchion (Human, Elf,
    Dark Elf, Kamael), Viper Fang (Orc) or Iron Hammer (Dwarf).
  - Mystics: Mage Staff, Tunic and Stockings of Devotion, Leather Helmet, Bracer, Boots.

## Runtime (to implement with inventories)

- **On creation:** add the items in list order. Set each timed item's expiry to creation time plus
  `lasts_minutes`, stored as an absolute time on the item instance, not on the character. Equip the items
  marked worn. Timed gear goes on after the permanent gear, which pushes the permanent pieces back into
  the bag. Nothing re-equips them later.
- **Grant rules:** every new character gets the kit. There is no account limit, level condition or flag.
- **Expiry:** online players are checked every second. At login, expired items go at once and the others
  are rescheduled. Nothing expires while the player is offline.
  - An expired item is unequipped if worn, then destroyed from the inventory or warehouse. The player sees
    "the limited-time item has disappeared".
  - Once per batch, at most one every 60 seconds, the player gets a mail from the server. Subject: "Your
    temporary items have expired". Body: the time is up; if it was the Recruit Kit, it was a 48-hour head
    start; put the starter gear back on; the Community Board shows where to go next.
- **Warnings:**
  - "Your temporary items disappear in <time>." is sent when the soonest expiry crosses 24 h, 6 h, 1 h and
    10 min. The time is rounded up to the whole minute.
  - At login, one warning with the time left is sent when any timed item remains. Thresholds already
    passed are not repeated.
  - The warning state lives in memory and is re-derived at login.
- **Restrictions on timed items:** never dropped on death, and they cannot be crystallized, augmented or
  sold on commission. The client shows the time left.
