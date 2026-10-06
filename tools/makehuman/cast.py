"""The cast: who Earth Two's people are and what they can look like.

This is the one file to edit to change the characters. make characters
builds assets/characters from it: people.py builds each person in PEOPLE
with the face in FACE and the clips in CLIPS and MIXAMO_CLIPS, traversal.py
authors their traversal clips (the ladder climb's hips move by
LADDER_HIPS), and wardrobe.py builds what each can wear from CATALOGUE, FACES and
SKINS, and the factions' looks from LOOKS (see the README). community.py
fetches the community clothes in COMMUNITY that the catalogue uses.
"""

# name: (gender 0 female .. 1 male, skin, clothes). The clothes are
# MakeHuman community assets: WojackOWL's Boxer Shorts (CC-BY; installed by
# hand as wojackowl_boxer_shorts, see the README), and wolgade's panties and
# top from the underwear01 pack (CC0).
PEOPLE = {
    "man": (1.0, "young_african_male", ["wojackowl_boxer_shorts"]),
    "woman": (0.0, "young_african_female", ["wolgade_female_panties_01", "wolgade_female_top_01"]),
}

# The face everyone gets, as MakeHuman targets and how much of each (a name
# with "?-" stands for both its "l-" and "r-" targets): stylised the way
# painted animation draws faces, with big eyes, high cheekbones over hollow
# cheeks, a narrow jaw and chin, a defined nose and full lips.
FACE = {
    "eyes/?-eye-scale-incr": 0.7,
    "eyes/?-eye-corner2-up": 0.35,
    "eyes/?-eye-height2-incr": 0.3,
    "eyebrows/eyebrows-angle-down": 0.35,
    "eyebrows/eyebrows-trans-down": 0.3,
    "cheek/?-cheek-bones-incr": 0.8,
    "cheek/?-cheek-volume-decr": 0.7,
    "cheek/?-cheek-inner-decr": 0.4,
    "head/head-diamond": 0.35,
    "head/head-fat-decr": 0.4,
    "chin/chin-triangle": 0.5,
    "chin/chin-prominent-incr": 0.35,
    "chin/chin-width-decr": 0.3,
    "nose/nose-point-width-decr": 0.5,
    "nose/nose-greek-incr": 0.3,
    "nose/nose-width2-decr": 0.3,
    "mouth/mouth-cupidsbow-incr": 0.6,
    "mouth/mouth-upperlip-volume-incr": 0.3,
    "mouth/mouth-lowerlip-volume-incr": 0.4,
    "neck/neck-scale-horiz-decr": 0.35,
    "neck/neck-scale-vert-incr": 0.25,
}

# Community clothes the catalogue uses besides the packs: folder name ->
# the asset's number on makehumancommunity.org. All CC-BY, credited in
# assets/characters/CREDITS.txt; community.py fetches and installs them.
COMMUNITY = {
    "elvs_male_coveralls_1": 2698,  # Elvs Male Coveralls 1, Elvaerwyn
    "elvs_male_trench_coat_1": 1815,  # Elvs Male Trench Coat1, Elvaerwyn
    "brkurt_big_desert_poncho": 64,  # Big Desert Poncho, brkurt
    "punkduck_uniform_jacket": 629,  # uniform jacket (train driver), punkduck
    "punkduck_uniform_pants": 630,  # uniform pants (train driver), punkduck
    "punkduck_goggles": 664,  # Goggles, punkduck
    "mathias_gredal_gas_mask": 2715,  # Gas Mask, Mathias_Gredal
    "wojackowl_surgical_mask": 3644,  # Surgical Mask, WojackOWL
}

# The factions' colours (docs/look-and-feel.md), in sRGB as textures hold
# them: an item given one is repainted in it, keeping its texture's folds,
# seams and wear.
CREW_ORANGE = (0.90, 0.47, 0.14)
CHARTER_NAVY = (0.12, 0.16, 0.32)
CHARTER_WHITE = (0.90, 0.90, 0.87)
REGISTRAR_BLACK = (0.16, 0.16, 0.17)
REGISTRAR_GREY = (0.40, 0.39, 0.37)
BLEACHED = (0.80, 0.72, 0.56)

# What each person can wear: slot -> [(item, display name)], or (item,
# display name, colour) for an item repainted in one of the colours above.
# Items are MakeHuman assets (system assets, the shirts01, pants01 and
# glasses01 packs, and COMMUNITY); "hair" items are in MPFB's hair folder,
# the rest in clothes. An outfit is a one-piece top and bottom: wearing one
# takes off the top and bottom, and the other way round. A coat is worn over
# either. A mask is worn over the face: out of the dome and the Hull the
# air's thin and dusty (docs/settlement.md).
CATALOGUE = {
    "man": {
        "hair": [("short02", "Short"), ("short04", "Crop"), ("short01", "Side part"),
                 ("afro01", "Afro"), ("braid01", "Braids")],
        "glasses": [("frankyaye_glasses_library_male", "Library"), ("kwnet_at_optical_glasses", "Optical"),
                    ("toigo_round_glasses_leopard", "Round"), ("punkduck_goggles", "Goggles")],
        "mask": [("mathias_gredal_gas_mask", "Rebreather"), ("wojackowl_surgical_mask", "Dust mask")],
        "top": [("joepal_crude_t-shirt_female", "T-shirt"), ("namuhekam_male_polo_shirt", "Polo"),
                ("toigo_fisherman_sweater", "Sweater"),
                ("punkduck_uniform_jacket", "Charter jacket"),
                ("toigo_fisherman_sweater", "Grey sweater", REGISTRAR_GREY)],
        "bottom": [("cortu_cargo_pants", "Cargo pants"), ("toigo_wool_pants", "Trousers"),
                   ("cortu_jeans_shorts", "Jeans shorts"),
                   ("punkduck_uniform_pants", "Charter trousers", CHARTER_WHITE)],
        "outfit": [("male_casualsuit01", "Denim shirt & jeans"), ("male_casualsuit03", "Striped shirt & jeans"),
                   ("male_casualsuit05", "Jacket & jeans"), ("male_casualsuit06", "White tee & jeans"),
                   ("male_worksuit01", "Overalls"), ("male_elegantsuit01", "Suit & tie"),
                   ("elvs_male_coveralls_1", "Crew coveralls", CREW_ORANGE),
                   ("brkurt_big_desert_poncho", "Desert poncho", BLEACHED)],
        "coat": [("elvs_male_trench_coat_1", "Registrar coat", REGISTRAR_BLACK)],
        "shoes": [("shoes05", "White trainers"), ("shoes06", "Blue trainers"), ("shoes02", "Grey sneakers"),
                  ("shoes01", "Brown brogues"), ("shoes04", "Black shoes"), ("shoes03", "Boots")],
    },
    "woman": {
        "hair": [("bob02", "Bob"), ("ponytail01", "Ponytail"), ("long01", "Long"),
                 ("afro01", "Afro"), ("braid01", "Braids"), ("short03", "Pixie")],
        "glasses": [("kwnet_at_optical_glasses", "Optical"), ("toigo_round_glasses_leopard", "Round"),
                    ("spamrakuen_sagerfrogs_glasses_02", "Frames"), ("punkduck_goggles", "Goggles")],
        "mask": [("mathias_gredal_gas_mask", "Rebreather"), ("wojackowl_surgical_mask", "Dust mask")],
        "top": [("joepal_crude_t-shirt_female", "T-shirt"), ("toigo_keyhole_tank_top", "Tank top"),
                ("toigo_camisole_top", "Camisole"), ("toigo_fisherman_sweater", "Sweater"),
                ("punkduck_uniform_jacket", "Charter jacket"),
                ("toigo_fisherman_sweater", "Grey sweater", REGISTRAR_GREY)],
        "bottom": [("cortu_cargo_pants", "Cargo pants"), ("toigo_harem_pants", "Harem pants"),
                   ("cortu_jeans_shorts", "Jeans shorts"),
                   ("punkduck_uniform_pants", "Charter trousers", CHARTER_WHITE)],
        "outfit": [("female_casualsuit01", "Tee & jeans"), ("female_casualsuit02", "Tee & shorts"),
                   ("female_sportsuit01", "Sportswear"), ("female_elegantsuit01", "Blouse & skirt"),
                   ("elvs_male_coveralls_1", "Crew coveralls", CREW_ORANGE),
                   ("brkurt_big_desert_poncho", "Desert poncho", BLEACHED)],
        "coat": [("elvs_male_trench_coat_1", "Registrar coat", REGISTRAR_BLACK)],
        "shoes": [("shoes05", "White trainers"), ("shoes06", "Blue trainers"), ("shoes04", "Black shoes"),
                  ("shoes03", "Boots")],
    },
}

# The factions' looks (docs/look-and-feel.md, docs/factions.md): a whole
# outfit to put on at once, so each reads at a glance, and so a contract can
# dress someone as one of them. Slot -> display name from CATALOGUE; None
# takes off what's in that slot, and slots left out (hair, face) are kept.
_LOOKS = {
    "Crew": {"outfit": "Crew coveralls", "coat": None, "shoes": "Boots", "mask": None, "glasses": None},
    "Charter": {"top": "Charter jacket", "bottom": "Charter trousers", "coat": None, "shoes": "Black shoes",
                "mask": None, "glasses": None},
    "Registrar": {"top": "Grey sweater", "bottom": "Cargo pants", "coat": "Registrar coat", "shoes": "Black shoes",
                  "mask": None},
    "Fringer": {"outfit": "Desert poncho", "coat": None, "shoes": "Boots", "mask": "Rebreather", "glasses": None},
    "Corvane": {"outfit": None, "coat": None, "shoes": "Black shoes", "mask": None, "glasses": None},
}
LOOKS = {
    "man": {**_LOOKS, "Registrar": {**_LOOKS["Registrar"], "bottom": "Trousers"},
            "Corvane": {**_LOOKS["Corvane"], "outfit": "Suit & tie"}},
    "woman": {**_LOOKS, "Corvane": {**_LOOKS["Corvane"], "outfit": "Blouse & skirt"}},
}

# The faces on offer besides the one the bodies are built with (FACE,
# "Standard" in the game): (file name, display name, the targets that differ
# from FACE). Each is a head to wear in place of the body's, so only
# the features can differ: the head's and neck's shapes are the body's, which
# keeps the neck's seam closed and the hair fitting.
FACES = [
    ("soft", "Soft", {
        "eyes/?-eye-scale-incr": 0.95, "eyes/?-eye-corner2-up": 0.15,
        "eyebrows/eyebrows-angle-down": 0.0, "eyebrows/eyebrows-trans-down": 0.0,
        "cheek/?-cheek-bones-incr": 0.4, "cheek/?-cheek-volume-decr": 0.1, "cheek/?-cheek-inner-decr": 0.0,
        "chin/chin-triangle": 0.2, "chin/chin-prominent-incr": 0.1,
        "nose/nose-point-up": 0.4, "nose/nose-greek-incr": 0.0, "nose/nose-scale-vert-decr": 0.3,
        "mouth/mouth-upperlip-volume-incr": 0.5, "mouth/mouth-lowerlip-volume-incr": 0.6,
    }),
    ("sharp", "Sharp", {
        "eyes/?-eye-scale-incr": 0.5, "eyes/?-eye-corner2-up": 0.7,
        "eyebrows/eyebrows-angle-down": 0.65,
        "cheek/?-cheek-bones-incr": 1.0, "cheek/?-cheek-volume-decr": 1.0, "cheek/?-cheek-inner-decr": 0.7,
        "chin/chin-triangle": 0.85, "chin/chin-prominent-incr": 0.55,
        "nose/nose-greek-incr": 0.55, "nose/nose-hump-incr": 0.4, "nose/nose-point-width-decr": 0.8,
        "mouth/mouth-upperlip-volume-incr": 0.1, "mouth/mouth-lowerlip-volume-incr": 0.15,
    }),
    ("strong", "Strong", {
        "eyes/?-eye-scale-incr": 0.4, "eyes/?-eye-corner2-up": 0.1,
        "eyebrows/eyebrows-angle-down": 0.5, "eyebrows/eyebrows-trans-down": 0.6,
        "cheek/?-cheek-bones-incr": 0.6, "cheek/?-cheek-volume-decr": 0.3,
        "chin/chin-triangle": 0.0, "chin/chin-width-decr": 0.0, "chin/chin-width-incr": 0.7,
        "chin/chin-bones-incr": 0.7, "chin/chin-prominent-incr": 0.5,
        "nose/nose-point-width-decr": 0.0, "nose/nose-width2-decr": 0.0, "nose/nose-scale-horiz-incr": 0.35,
        "nose/nose-hump-incr": 0.3,
        "mouth/mouth-scale-horiz-incr": 0.35,
    }),
]
# The skins on offer, by MakeHuman skin name without the _male/_female.
SKINS = [("young_african", "Dark"), ("middleage_african", "Dark, older"),
         ("young_asian", "Light brown"), ("middleage_asian", "Light brown, older"),
         ("young_caucasian", "Fair"), ("middleage_caucasian", "Fair, older")]

# The library clips each person gets. Each adds to the file, so only the
# ones the game uses.
CLIPS = [
    "Idle_Loop", "Idle_Talking_Loop", "Walk_Loop", "Walk_Formal_Loop",
    "Jog_Fwd_Loop", "Sprint_Loop", "Jump_Start", "Jump_Loop", "Jump_Land",
    "Interact", "PickUp_Table", "Punch_Jab", "Punch_Cross", "Push_Loop",
    "Crouch_Idle_Loop", "Crouch_Fwd_Loop", "Roll", "Hit_Chest", "Death01",
    "Driving_Loop", "Sitting_Enter", "Sitting_Idle_Loop", "Sitting_Exit",
    "Pistol_Idle_Loop", "Pistol_Aim_Neutral", "Pistol_Shoot", "Fixing_Kneeling",
    "Dance_Loop",
]

# Mixamo clips: motion capture, so the everyday moves (standing, walking,
# running, jumping) look natural where the library's are stylised. Each is a
# "Without Skin" download in MIXAMO_DIR named after the clip, e.g.
# "Walking.fbx". Mixamo's terms allow them in games but not redistributed as
# raw files, so the downloads stay out of the repo.
MIXAMO_CLIPS = ["Breathing Idle", "Walking", "Running", "Jumping", "Running Jump", "Picking Up", "Punching"]

# How much the hips and spine move climbing a ladder: the share of how they
# move in Mixamo's "Climbing Ladder" (see traversal.ladder_moves). 1 is the
# capture's, 0 a still waist.
LADDER_HIPS = 1.
