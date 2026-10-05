"""Author traversal clips on the existing MakeHuman rig, without changing meshes.

blender --background --factory-startup --python tools/makehuman/traversal.py -- assets/characters [MIXAMO_DIR]
The donor export is merged by merge_animations.py; originals keep their meshes,
skins, materials, bind poses and existing animation bytes. The wall kick,
vault and mantle are motion capture, from the Mixamo downloads in MIXAMO_DIR
(see WALL_RUN and VAULT).
"""
import math
import os
import sys
import bpy
from mathutils import Quaternion, Vector, Matrix

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cast import LADDER_HIPS  # noqa: E402

# Rails are 48 cm apart with a 7 cm square section, 37 cm ahead of the axis.
# Position the palm, then derive the wrist for each body's hand proportions.
RAIL_HALF_WIDTH = .24
RAIL_DEPTH = -.37
RAIL_HALF_SECTION = .035
# Each foot stands this far to its side of the middle of a rung.
FOOT_APART = .12
# Rungs are RUNG apart. At the start of a climb cycle the left foot stands
# RUNG_FOOT above the body's own ground, and the game starts the cycle
# ladderFoot (character/traversal.go) up a ladder: the two put the first
# rung 30 cm up, and the body on the right foot at the top, where the hop
# off begins.
RUNG, RUNG_FOOT = .3, .09
# On a ladder the body hangs off the rails with long arms and legs, not
# curled up to it. The hips ride as high as leaves the lower leg LADDER_REACH
# of its length at its straightest (see ladder_stance), and LADDER_BACK
# further from the ladder; the hands hold the rails around LADDER_GRIP above
# the shoulders; the trunk leans LADDER_LEAN degrees in towards them; and the
# elbows point down and a little out, by LADDER_ELBOW (a point they bend
# towards, mirrored for the right).
LADDER_REACH, LADDER_BACK, LADDER_GRIP, LADDER_LEAN = .95, .1, .05, 18
# How far in front of the rails and rungs the middle of a knee stays.
KNEE = .08
LADDER_ELBOW = (.5, .35, .6)
# Where the hands hold the rails and how far the hips are raised: set for
# each body by ladder_stance.
RAIL_GRIP_HEIGHT = 1.5
LADDER_HIGH = .07

# The wall kick is the last step of Mixamo's "Wall Run" (an FBX "Without
# Skin" download in MIXAMO, like the clips people.py takes): the left foot
# taps the wall on the body's left and pushes off it, and the body turns to
# face away. The steps up the wall before it are left out. There the wall is
# on the +X side and the body ends up facing -X; WALL_KICK_TURN brings that to
# the rig's front (-Y), so the game holds the body facing away from the wall.
# WallKickRight is its mirror image, for a wall on the right.
MIXAMO = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "build", "mixamo")
WALL_RUN = "Wall Run"
WALL_KICK_FRAMES = (21, 29)
WALL_KICK_TURN = Quaternion((0, 0, 1), math.radians(90))

# Landing from a wall kick is the touchdown of Mixamo's "Falling To Landing":
# from the feet meeting the ground, down into the knees and up again, played
# a little faster than the capture so it's over quickly.
LANDING = "Falling To Landing"
LANDING_FRAMES = (9, 27)

# The vault and mantle are Mixamo's "Vault Over Box" (downloaded the same
# way), which already runs the rig's way: a run up to a box, a hop onto it off
# the left foot with both hands down, a couple of strides across and a jump
# down. VAULT_KEYS maps the controller's phases (lift to .4, cross to .85,
# then landing) to its frames; the mantle stops on top and straightens up.
# The capture's box is VAULT_BOX high, by its ankles. The controller goes
# straight up, across and down, and the body leaps: the hips keep the
# capture's path less the controller's, so over a box like the capture's the
# two add up to the capture. The dive into cover after the landing is left out.
VAULT = "Vault Over Box"
VAULT_KEYS = {"Vault": ((0, 11), (.4, 23), (.85, 39), (1, 45)), "Mantle": ((0, 11), (.4, 23), (1, 33))}
VAULT_BOX = .89
VAULT_HOVER = .08  # how far above the obstacle the controller carries the body
MANTLE_STAND = .6  # the share of the mantle after which it straightens up

# The slide is Mixamo's "Running Slide" (downloaded the same way): out of a
# run it drops onto its hips, the left leg out ahead and the right folded
# under, leaning back and over onto the right hand, then gets up and runs on.
# SLIDE_FRAMES is from the drop to back on its feet, mid-stride, where the
# game runs on if it's still sprinting (see slideRelease in
# character/traversal.go). It runs the rig's way; the controller carries it.
SLIDE = "Running Slide"
SLIDE_FRAMES = (5, 37)

# Climbing stairs is Mixamo's "Ascending Stairs" (downloaded the same way):
# STAIRS_FRAMES is one cycle of it, a step with each foot. Its stairs are
# steeper and shallower than ours, 0.235 m up and 0.25 m deep, so the feet
# are moved (by leg IK) to step as far as ours: STAIR_RISE up and
# STAIR_TREAD on (the kit's stairs, tools/world/hull_kit.py), about where
# they are on average. The climb itself is the controller's, up the stairs'
# ramp: the hips keep only how they sway about it. The game sets the clip
# by how high the feet are (character/animate.go), and plays it backwards
# coming down.
STAIRS = "Ascending Stairs"
STAIRS_FRAMES = (1, 41)
STAIR_RISE, STAIR_TREAD = .3, .5

# The ladder climb is authored (hands on the rails, feet on the rungs), but
# its hips and spine move as in Mixamo's "Climbing Ladder" (downloaded the
# same way), which climbs facing the other way, two rungs a cycle like ours:
# the hips turn into each step and shift over the planted foot, and the body
# rises in surges. Only how they move about their average is taken, so the
# authored lean and reach stay. The capture's left foot leaves its rung
# LADDER_PHASE of a cycle in; ours leaves at the start.
LADDER = "Climbing Ladder"
LADDER_FRAMES = (1, 24)
LADDER_PHASE = 4/23
LADDER_TURN = Quaternion((0, 0, 1), math.pi)
LADDER_BONES = ("pelvis", "spine_01", "spine_02", "spine_03", "neck_01", "head")

# Leaving a ladder at its top starts at phase .5 of the climb, the left foot
# on a rung four below the top one and the hands at the ends of the rails.
# The body climbs on until LADDER_EXIT_CLIMB of the clip, by LADDER_EXIT_RISE,
# as the hands leave the rails for the landing; from there to LADDER_EXIT_UP
# it is the hop onto the box out of VAULT (frames LADDER_EXIT_FRAMES), the
# landing at the box's height, and then it stands up where it is, the capture
# slowing to a stop under it. That box is nearer than a landing behind a
# ladder, so the hips lean in by LADDER_EXIT_LEAN; they end LADDER_EXIT_ACROSS
# ahead, just onto the landing. The controller gets there by LADDER_EXIT_CROSS,
# which doesn't show: the hips keep the capture's path less the controller's
# (see character/traversal.go).
# The shares fall on whole frames: the controller changes pace exactly there,
# and a change smeared between two frames shows as a twitch.
LADDER_EXIT_CLIMB, LADDER_EXIT_UP, LADDER_EXIT_CROSS = 42/90, 63/90, 81/90
LADDER_EXIT_RISE, LADDER_EXIT_LEAN, LADDER_EXIT_ACROSS = .84, .3, 1.
LADDER_EXIT_FRAMES = (13, 23)


def hop_frame(t):
    """The frame of VAULT a ladder exit shows at t: a steady pace through the
    hop, and before it under the blend in, then easing to a stop."""
    pace = (LADDER_EXIT_FRAMES[1]-LADDER_EXIT_FRAMES[0])/(LADDER_EXIT_UP-LADDER_EXIT_CLIMB)
    if t <= LADDER_EXIT_UP:
        return LADDER_EXIT_FRAMES[0]+pace*(t-LADDER_EXIT_CLIMB)
    left = 1-LADDER_EXIT_UP
    return LADDER_EXIT_FRAMES[1]+pace*left/2*(1-((1-t)/left)**2)


# Mirrored clips: each of the rig's own clips here gets a copy with left and
# right swapped, under the name it maps to. Mixamo's "Punching" throws the
# left hand; the game alternates it with its mirror, the right.
MIRRORED = {"Punching": "Punching Mirrored"}


DURATIONS = {"Slide": .8, "Ladder": 1., "Vault": 1., "Mantle": .8, "WallKick": (WALL_KICK_FRAMES[1]-WALL_KICK_FRAMES[0])/30,
             "WallKickRight": (WALL_KICK_FRAMES[1]-WALL_KICK_FRAMES[0])/30, "WallKickFall": 1.2, "WallKickFallRight": 1.2, "WallLand": .45, "Crouch": 1., "StandUp": .28, "LadderExit": 3., "Fall": .9,
             "StairsUp": (STAIRS_FRAMES[1]-STAIRS_FRAMES[0])/30}


def smooth(a, b, t):
    t = max(0., min(1., t)); t = t*t*(3-2*t)
    return a+(b-a)*t


def pose(name, t):
    """Angles about Blender world X: forward is -Y, up is Z.
    All motion is in-place; the fixed-step controller supplies translation.
    """
    p = {}; z = 0.; lean = 0.; back = 0.
    if name in ("Crouch", "StandUp"):
        recover = smooth(0, 1, t) if name == "StandUp" else 0
        z = -.56*(1-recover)
        lean = 52*(1-recover)
    elif name in ("Ladder", "LadderExit"):
        lean = LADDER_LEAN; back = LADDER_BACK  # the height is LADDER_HIGH, in metres
    p["spine_01"] = lean*.5; p["spine_02"] = lean*.3; p["spine_03"] = lean*.2
    return p, z, back



def orient(bone, world_rotation):
    # Change rotation only. Assigning pose matrices can accumulate scale/shear
    # through finger chains while baking hundreds of frames.
    parent_frame = bone.matrix.to_quaternion() @ bone.rotation_quaternion.inverted()
    bone.rotation_quaternion = (parent_frame.inverted() @ world_rotation).normalized()
    bpy.context.view_layer.update()


def aim(bone, target):
    """Rotate about the joint without changing bind translations or scales."""
    delta = (bone.tail-bone.head).rotation_difference(Vector(target)-bone.head)
    orient(bone,delta @ bone.matrix.to_quaternion())


def limb(rig, upper, lower, end, target, pole, end_rotation, behind=None, aside=None):
    """Two bones from where upper starts to target, bending towards pole.
    With behind, the joint between them stays that side (in Y) of it,
    swinging round towards aside as far as it has to: a knee that turns out
    rather than going through a ladder."""
    a, b, tip = (rig.pose.bones[n] for n in (upper, lower, end))
    start = a.head.copy(); target = Vector(target)
    length_a, length_b = a.length, b.length
    delta = target-start
    distance = max(.001, min(delta.length, length_a+length_b-.001))
    direction = delta.normalized()
    bend = Vector(pole)-start
    bend = (bend-direction*bend.dot(direction)).normalized()
    projection = (length_a*length_a-length_b*length_b+distance*distance)/(2*distance)
    height = math.sqrt(max(0, length_a*length_a-projection*projection))
    if behind is not None:
        out = Vector(aside)-direction*Vector(aside).dot(direction)
        out = (out-bend*out.dot(bend)).normalized()
        forward = bend
        for degrees in range(0, 91, 2):
            bend = forward*math.cos(math.radians(degrees))+out*math.sin(math.radians(degrees))
            if (start+direction*projection+bend*height).y >= behind:
                break
    aim(a, start+direction*projection+bend*height)
    aim(b, start+direction*distance)
    orient(tip,end_rotation)


def rung(t, high=False):
    """One contact advances two rungs per cycle.
    The planted half-cycle subtracts root travel exactly, including in reverse.
    """
    if high:
        return rung((t+.5)%1, False)
    return RUNG_FOOT + 2*RUNG*smooth(0, 1, t/.5) - 2*RUNG*t


def rail_hand(side, rest):
    """Thumb up, fingers forward, palm against the rail's outside face.

    The wrist alone is not a contact marker. Align the metacarpal frame and
    anchor the middle knuckle, so different hand sizes share the same grip.
    """
    sign = 1 if side == "l" else -1
    wrist, rotation = rest["hand_"+side]
    knuckle = rest["middle_01_"+side][0]
    along = (knuckle-wrist).normalized()
    across = rest["index_01_"+side][0]-rest["pinky_01_"+side][0]
    across = (across-along*across.dot(along)).normalized()
    source = Matrix((along,across,along.cross(across))).transposed()
    forward, up = Vector((0,-1,0)), Vector((0,0,1))
    target = Matrix((forward,up,forward.cross(up))).transposed()
    turn = (target @ source.transposed()).to_quaternion()
    anchor = Vector((sign*(RAIL_HALF_WIDTH+RAIL_HALF_SECTION+.017), -.38, RAIL_GRIP_HEIGHT))
    return anchor-turn @ (knuckle-wrist), turn @ rotation


def rail_grip(rig, side, strength):
    """Wrap fingers in the horizontal plane; oppose the thumb across the rail."""
    if strength <= 0:
        return
    sign = 1 if side == "l" else -1
    for finger in ("index", "middle", "ring", "pinky"):
        for joint in (1,2,3):
            bone = rig.pose.bones.get(f"{finger}_{joint:02d}_{side}")
            if bone is None:
                continue
            # Fit each phalanx around the rail's cross-section using its actual
            # length, rather than bending about an assumed local bone axis.
            x = sign*bone.head.x-RAIL_HALF_WIDTH
            y = bone.head.y-RAIL_DEPTH
            radius = math.hypot(x,y)
            wrap_radius = .060
            cosine = (radius*radius+wrap_radius*wrap_radius-bone.length*bone.length)/(2*radius*wrap_radius)
            angle = math.atan2(y,x)-math.acos(max(-1,min(1,cosine)))
            target = Vector((sign*(RAIL_HALF_WIDTH+wrap_radius*math.cos(angle)),
                             RAIL_DEPTH+wrap_radius*math.sin(angle),bone.head.z))
            natural = (bone.tail-bone.head).normalized()
            aim(bone,bone.head+natural.lerp((target-bone.head).normalized(),strength).normalized())
    for joint, offset, y in ((1,-.015,-.311),(2,-.049,-.317),(3,-.051,-.365)):
        bone = rig.pose.bones["thumb_%02d_%s" % (joint,side)]
        target = Vector((sign*(RAIL_HALF_WIDTH+offset),y,rig.pose.bones["middle_01_"+side].head.z+.035))
        natural = (bone.tail-bone.head).normalized()
        aim(bone,bone.head+natural.lerp((target-bone.head).normalized(),strength).normalized())


def contacts(rig, name, t, rest):
    if name not in ("Crouch", "StandUp", "Ladder", "LadderExit"):
        return
    recover = smooth(0,1,t) if name == "StandUp" else 0
    for side, sign in (("l",1),("r",-1)):
        foot = Vector((sign*.18, -.035, .085))
        foot_rotation = rest["foot_"+side][1]
        hand = Vector((sign*.27, -.34, .48))
        hand_rotation = rest["hand_"+side][1]
        elbow = (sign*.85,.05,.9)
        if name == "StandUp":
            foot = foot.lerp(rest["foot_"+side][0],recover)
            hand = hand.lerp(rest["hand_"+side][0],recover)
        if name == "Ladder":
            foot = Vector((sign*FOOT_APART, -.285, rung(t,side=="r")+.07))
            hand, hand_rotation = rail_hand(side,rest)
            # Alternate a supporting palm with a smooth upward slide/regrip.
            # Subtracting body travel holds the supporting hand fixed in world space.
            hand.z += rung(t,side=="l")-RUNG_FOOT-RUNG/2
            # Only feet target discrete rungs.
            # Swing feet clear the bar; planted feet stay at its surface.
            phase = (t + (.5 if side=="r" else 0))%1
            foot.y += .10*math.sin(math.pi*min(1,phase/.5)) if phase<.5 else 0
        if name == "LadderExit":
            # The feet climb on; the hands stay at the ends of the rails as
            # the body rises past them. author() blends the hop in.
            climb = min(1, t/LADDER_EXIT_CLIMB)
            cycle = (.5+LADDER_EXIT_RISE/(2*RUNG)*climb)%1
            foot = Vector((sign*FOOT_APART, -.285, rung(cycle,side=="r")+.07))
            phase = (cycle + (.5 if side=="r" else 0))%1
            foot.y += .10*math.sin(math.pi*min(1,phase/.5)) if phase<.5 else 0
            hand, hand_rotation = rail_hand(side,rest)
            hand.z += (-RUNG/2 if side=="l" else RUNG/2)-LADDER_EXIT_RISE*climb
        if name in ("Ladder", "LadderExit"):
            # The knees come up to the rungs and no further: they turn out.
            limb(rig,"thigh_"+side,"calf_"+side,"foot_"+side,foot,(sign*.35,-1,.45),foot_rotation,RAIL_DEPTH+RAIL_HALF_SECTION+KNEE,(sign,0,0))
        else:
            limb(rig,"thigh_"+side,"calf_"+side,"foot_"+side,foot,(sign*.35,-1,.45),foot_rotation)
        if name in ("Ladder", "LadderExit"):
            elbow = (sign*LADDER_ELBOW[0], LADDER_ELBOW[1], LADDER_ELBOW[2])
        limb(rig,"upperarm_"+side,"lowerarm_"+side,"hand_"+side,hand,elbow,hand_rotation)
        if name in ("Ladder", "LadderExit"):
            rail_grip(rig,side,1)

def mixamo_bones():
    """Mixamo's bone names mapped to the game engine rig's."""
    names = {"Hips": "pelvis", "Spine": "spine_01", "Spine1": "spine_02", "Spine2": "spine_03",
             "Neck": "neck_01", "Head": "head"}
    for side, s in (("Left", "l"), ("Right", "r")):
        names.update({side+"Shoulder": "clavicle_"+s, side+"Arm": "upperarm_"+s,
                      side+"ForeArm": "lowerarm_"+s, side+"Hand": "hand_"+s,
                      side+"UpLeg": "thigh_"+s, side+"Leg": "calf_"+s,
                      side+"Foot": "foot_"+s, side+"ToeBase": "ball_"+s})
        for finger in ("Thumb", "Index", "Middle", "Ring", "Pinky"):
            for i in (1, 2, 3):
                names["%sHand%s%d" % (side, finger, i)] = "%s_%02d_%s" % (finger.lower(), i, s)
    return {"mixamorig:"+k: v for k, v in names.items()}


# Bones that keep their own rest pose when the rig is lined up with Mixamo's
# T-pose: both skeletons stand upright, each spine in its own way.
UPRIGHT = {"pelvis", "spine_01", "spine_02", "spine_03", "neck_01", "head", "clavicle_l", "clavicle_r"}
SPINE = ("spine_01", "spine_02", "spine_03")


def palm(along, across):
    """A rotation whose Y axis is along and X axis lies toward across."""
    y = along.normalized()
    x = (across-y*across.dot(y)).normalized()
    return Matrix((x, y, x.cross(y))).transposed().to_quaternion()


def wall_kick(rig, folder):
    """The wall kick's frames: WALL_KICK_FRAMES of WALL_RUN, turned to face
    away from the wall."""
    frames = capture(rig, folder, WALL_RUN, range(WALL_KICK_FRAMES[0], WALL_KICK_FRAMES[1]+1), WALL_KICK_TURN)[0]
    # One foot taps the wall. In the capture the right leg is still coming off
    # the step before, by the wall too, so it stays tucked under the hips
    # instead, as it ends up.
    for pose in frames[:-1]:
        carry = pose["pelvis"] @ frames[-1]["pelvis"].inverted()
        for n in ("thigh_r", "calf_r", "foot_r", "ball_r"):
            pose[n] = carry @ frames[-1][n]
    return frames


def vault_frame(name, t):
    """The frame of VAULT that name shows at t, by VAULT_KEYS."""
    keys = VAULT_KEYS[name]
    for (t0, f0), (t1, f1) in zip(keys, keys[1:]):
        if t <= t1:
            return f0+(f1-f0)*(t-t0)/(t1-t0)
    return keys[-1][1]


def carried(name, t):
    """How far the controller has taken the body at t, up the obstacle and
    across it, each 0 to 1."""
    across = max(0, min(1, (t-.4)/.45))
    if t < .4:
        return t/.4, across
    return (1-(t-.85)/.15 if name == "Vault" and t > .85 else 1), across


def capture(rig, folder, clip, at, turn):
    """Frames at of the Mixamo clip, each {bone: armature-space rotation},
    parents first, carried over to rig bone by bone, relative to a reference
    pose (rig posed like Mixamo's rest), in place and turned by turn; and
    where the hips are at each, and a metre of the capture, in rig units."""
    objects, actions = set(bpy.data.objects), set(bpy.data.actions)
    bpy.ops.import_scene.fbx(filepath=os.path.join(folder, clip+".fbx"))
    new = [o for o in bpy.data.objects if o not in objects]
    source = next(o for o in new if o.type == "ARMATURE")
    pairs = {t: s for s, t in mixamo_bones().items() if s in source.pose.bones and t in rig.pose.bones}
    order = sorted(pairs, key=lambda n: len(rig.data.bones[n].parent_recursive))
    into = rig.matrix_world.inverted() @ source.matrix_world

    def at_rest(name):
        return into @ source.data.bones[pairs[name]].head_local

    for b in rig.pose.bones:
        b.rotation_mode = "QUATERNION"; b.rotation_quaternion = Quaternion()
    bpy.context.view_layer.update()
    for name in order:
        kids = [c.name for c in rig.data.bones[name].children if c.name in pairs]
        hand = name.startswith("hand_")
        if name in UPRIGHT or len(kids) != 1 and not hand:
            continue  # the ends of chains follow their parents
        bone = rig.pose.bones[name]
        child = "middle_01_"+name[-1] if hand else kids[0]
        want, have = at_rest(child)-at_rest(name), rig.pose.bones[child].head-bone.head
        line = have.rotation_difference(want)  # not turn: that's the argument
        if hand:  # the palms turn the same way too
            a, b = "index_01_"+name[-1], "pinky_01_"+name[-1]
            line = palm(want, at_rest(b)-at_rest(a)) @ palm(have, rig.pose.bones[b].head-rig.pose.bones[a].head).inverted()
        orient(bone, line @ bone.matrix.to_quaternion())
    reference = {n: rig.pose.bones[n].matrix.to_quaternion() for n in order}
    source_rest = {n: (into @ source.data.bones[s].matrix_local).to_quaternion() for n, s in pairs.items()}
    torso = at_rest("neck_01")-at_rest("spine_01"), rig.data.bones["neck_01"].head_local-rig.data.bones["spine_01"].head_local

    metre = rig.data.bones["pelvis"].head_local.z/at_rest("pelvis").z
    frames, hips = [], []
    for f in at:
        bpy.context.scene.frame_set(int(f), subframe=f-int(f))
        posed = {n: into @ source.pose.bones[s].matrix for n, s in pairs.items()}
        want = {n: turn @ posed[n].to_quaternion() @ source_rest[n].inverted() @ reference[n] for n in order}
        for n in order:
            orient(rig.pose.bones[n], want[n])
        # The same spine rotations lean MakeHuman's torso further than
        # Mixamo's; turn the spine to lean it as much as the capture does.
        lean = torso[0].rotation_difference(posed["neck_01"].to_translation()-posed["spine_01"].to_translation())
        have = rig.pose.bones["neck_01"].head-rig.pose.bones["spine_01"].head
        tilt = have.rotation_difference(turn @ (lean @ torso[1]))
        for n in order:
            orient(rig.pose.bones[n], tilt @ want[n] if n in SPINE else want[n])
        frames.append({n: rig.pose.bones[n].matrix.to_quaternion() for n in order})
        hips.append(turn @ posed["pelvis"].to_translation()*metre)

    for o in new:
        bpy.data.objects.remove(o, do_unlink=True)
    for action in list(bpy.data.actions):
        if action not in actions: bpy.data.actions.remove(action)
    return frames, hips, metre


# The fall after a wall kick loops the kick's last pose, kept loose: each of
# these bones sways about a body axis (X side to side, Y front to back) by so
# many degrees, so many times a loop, from a phase, carrying its limb along.
FALL_SWAY = {"spine_01": ((1, 0, 0), 2.5, 1, 0), "spine_03": ((0, 1, 0), 2, 1, .3),
             "upperarm_l": ((0, 1, 0), 6, 2, .1), "upperarm_r": ((0, 1, 0), -6, 2, .3),
             "lowerarm_l": ((1, 0, 0), 5, 2, .25), "lowerarm_r": ((1, 0, 0), 5, 2, .45),
             "thigh_l": ((1, 0, 0), 6, 1, 0), "thigh_r": ((1, 0, 0), 6, 1, .5),
             "calf_l": ((1, 0, 0), -8, 1, .15), "calf_r": ((1, 0, 0), -8, 1, .65)}


# Falling from higher than a jump, the body hangs in the jump's pose just
# before touchdown (FALL_POSE: the rig's clip and how many seconds into it,
# where the game holds the jump; see Clip.Land in character/roster.go) and
# swings its legs, one forward as the other goes back, the arms swinging
# against them, until it lands. LEG_SWING says how, as FALL_SWAY does. The
# legs start even (the pose's halfway to its mirror image), to swing as far
# either way.
FALL_POSE = ("Jumping", 1.05)
LEGS = ("thigh", "calf", "foot", "ball")
LEG_SWING = {"thigh_l": ((1, 0, 0), 28, 1, 0), "thigh_r": ((1, 0, 0), 28, 1, .5),
             "calf_l": ((1, 0, 0), 18, 1, .25), "calf_r": ((1, 0, 0), 18, 1, .75),
             "upperarm_l": ((1, 0, 0), 10, 1, .5), "upperarm_r": ((1, 0, 0), 10, 1, 0),
             "spine_01": ((0, 1, 0), 3, 1, 0)}


def pose_at(rig, action, seconds, fps):
    """The rig's pose seconds into action, imported at fps: each bone's
    armature-space rotation, parents first, and where the hips are. It
    leaves the rig posed as it was."""
    was = {b.name: (b.rotation_mode, b.rotation_quaternion.copy(), b.location.copy(), b.scale.copy()) for b in rig.pose.bones}
    a = bpy.data.actions[action]
    rig.animation_data.action = a
    if a.slots: rig.animation_data.action_slot = a.slots[0]
    at = a.frame_range[0]+seconds*fps
    bpy.context.scene.frame_set(int(at), subframe=at-int(at))
    bpy.context.view_layer.update()
    order = sorted(rig.pose.bones, key=lambda b: len(b.parent_recursive))
    pose = {b.name: b.matrix.to_quaternion() for b in order}
    hips = rig.pose.bones["pelvis"].head.copy()
    rig.animation_data.action = None
    for b in rig.pose.bones:
        b.rotation_mode, b.rotation_quaternion, b.location, b.scale = was[b.name]
    bpy.context.view_layer.update()
    return pose, hips


def falling(rig, pose, t, sway=None):
    """pose swaying as sway (FALL_SWAY by default) says, t of the way
    through its loop."""
    out = dict(pose)
    for bone, (axis, degrees, cycles, phase) in (sway or FALL_SWAY).items():
        sway = Quaternion(axis, math.radians(degrees)*math.sin(2*math.pi*(cycles*t+phase)))
        for n in out:
            if n == bone or any(p.name == bone for p in rig.data.bones[n].parent_recursive):
                out[n] = sway @ out[n]
    return out


def mirrored(rig, pose):
    """pose with left and right swapped: each bone is turned from its rest
    the way its opposite is, reflected."""
    def rest(n): return rig.data.bones[n].matrix_local.to_quaternion()
    out = {}
    for name in pose:
        other = name[:-1]+{"l": "r", "r": "l"}[name[-1]] if name[-2:] in ("_l", "_r") else name
        turn = pose[other] @ rest(other).inverted()
        out[name] = Quaternion((turn.w, turn.x, -turn.y, -turn.z)) @ rest(name)
    return out


def mirror_clip(rig, source, name, fps):
    """Bakes the rig's action source, imported at fps, mirrored (see
    mirrored) as name: the same frames, with the hips across the middle the
    other way."""
    scene = bpy.context.scene
    action = bpy.data.actions[source]
    rig.animation_data_create()
    rig.animation_data.action = action
    if action.slots: rig.animation_data.action_slot = action.slots[0]
    # Keyed 30 times a second, like the clips author makes.
    start, end = action.frame_range
    step = fps/30
    frames = round((end-start)/step)
    poses = []
    for f in range(frames+1):
        at = start+f*step
        scene.frame_set(int(at), subframe=at-int(at))
        bpy.context.view_layer.update()
        poses.append(({b.name: b.matrix.to_quaternion() for b in rig.pose.bones}, rig.pose.bones["pelvis"].head.copy()))
    rig.animation_data.action = None
    out = bpy.data.actions.new(name)
    out.use_fake_user = True
    rig.animation_data.action = out
    order = sorted(rig.pose.bones, key=lambda b: len(b.parent_recursive))
    for f, (pose, hips) in enumerate(poses):
        for b in rig.pose.bones:
            b.rotation_mode = "QUATERNION"; b.rotation_quaternion = Quaternion(); b.location = (0, 0, 0)
        bpy.context.view_layer.update()
        flipped = mirrored(rig, pose)
        for b in order:
            orient(b, flipped[b.name])
        pelvis = rig.pose.bones["pelvis"]
        pelvis.location += pelvis.bone.matrix_local.to_quaternion().inverted() @ (Vector((-hips.x, hips.y, hips.z))-pelvis.head)
        bpy.context.view_layer.update()
        for b in rig.pose.bones:
            b.keyframe_insert("rotation_quaternion", frame=f+1)
            if b.name == "pelvis": b.keyframe_insert("location", frame=f+1)
    if out.slots:
        bag = out.layers[0].strips[0].channelbag(out.slots[0])
        for curve in bag.fcurves:
            for key in curve.keyframe_points: key.interpolation = 'LINEAR'
    rig.animation_data.action = None
    for b in rig.pose.bones:
        b.rotation_quaternion = Quaternion(); b.location = (0, 0, 0)


def ladder_stance(rig, rest):
    """Sets LADDER_HIGH and RAIL_GRIP_HEIGHT for rig, whose bones stand at
    rest: its hips as high over the rungs as its legs allow, and its hands
    by its shoulders."""
    global LADDER_HIGH, RAIL_GRIP_HEIGHT
    leg = rig.data.bones["thigh_l"].length+rig.data.bones["calf_l"].length
    hip = rest["thigh_l"][0]
    # The foot at its lowest, as contacts has it: about to leave its rung.
    foot = Vector((FOOT_APART, -.285, rung(0)+.07))
    away = math.hypot(foot.x-hip.x, foot.y-(hip.y+LADDER_BACK))
    LADDER_HIGH = foot.z+math.sqrt((LADDER_REACH*leg)**2-away**2)-hip.z
    RAIL_GRIP_HEIGHT = rest["upperarm_l"][0].z+LADDER_HIGH+LADDER_GRIP


def ladder_moves(rig, folder):
    """How the hips and spine move in LADDER, as a function of the climb's
    phase: each of LADDER_BONES' rotation from its average over the cycle
    (armature space), and how far the hips are from where a steady climb
    would have them."""
    first, last = LADDER_FRAMES
    poses, hips, _ = capture(rig, folder, LADDER, range(first, last+1), LADDER_TURN)
    steps = last-first
    rise = hips[-1]-hips[0]
    offsets = [hips[i]-hips[0]-rise*(i/steps) for i in range(steps+1)]
    centre = sum(offsets[:-1], Vector())/steps
    average = {}
    for n in LADDER_BONES:
        total = Quaternion((0, 0, 0, 0))
        for pose in poses[:-1]:
            q = pose[n]
            total += q if q.dot(poses[0][n]) >= 0 else -q
        average[n] = total.normalized()

    def at(phase):
        x = (phase+LADDER_PHASE)%1*steps
        i = min(int(x), steps-1)
        turns = {n: (poses[i][n] @ average[n].inverted()).slerp(poses[i+1][n] @ average[n].inverted(), x-i) for n in LADDER_BONES}
        return turns, offsets[i].lerp(offsets[i+1], x-i)-centre
    return at


def climb(rig, moves, phase, share):
    """Moves the hips and spine as moves (see ladder_moves) has them at phase,
    by share of it."""
    turns, offset = moves(phase)
    bones = [rig.pose.bones[n] for n in LADDER_BONES]
    # Each from where it is now, whatever its parent goes on to do.
    posed = [b.matrix.to_quaternion() for b in bones]
    for b, q in zip(bones, posed):
        orient(b, Quaternion().slerp(turns[b.name], share) @ q)
    pelvis = rig.pose.bones["pelvis"]
    pelvis.location += pelvis.bone.matrix_local.to_quaternion().inverted() @ (offset*share)
    bpy.context.view_layer.update()


def stairs_cycle(rig, folder, base, rest):
    """The stair climb's frames: the capture's pose at each, how far the hips
    sway from their steady climb, and where each foot goes and how it's
    turned, stretched to our steps."""
    frames = round(DURATIONS["StairsUp"]*30)
    poses, hips, metre = capture(rig, folder, STAIRS, [STAIRS_FRAMES[0]+(STAIRS_FRAMES[1]-STAIRS_FRAMES[0])*f/frames for f in range(frames+1)], Quaternion())
    climb = hips[-1]-hips[0]
    sway = [hips[f]-hips[0]-climb*(f/frames) for f in range(frames+1)]
    feet = []
    for f in range(frames+1):
        for b in rig.pose.bones:
            q, loc, scale = base[b.name]
            b.rotation_mode = "QUATERNION"; b.rotation_quaternion = q.copy(); b.location = loc.copy(); b.scale = scale.copy()
        bpy.context.view_layer.update()
        for n, q in poses[f].items():
            orient(rig.pose.bones[n], q)
        pelvis = rig.pose.bones["pelvis"]
        pelvis.location += pelvis.bone.matrix_local.to_quaternion().inverted() @ (rest["pelvis"][0]+sway[f]-pelvis.head)
        bpy.context.view_layer.update()
        feet.append({s: (rig.pose.bones["foot_"+s].head.copy(), rig.pose.bones["foot_"+s].matrix.to_quaternion()) for s in "lr"})
    # Stretch each foot's way about its average, to step our steps: a cycle
    # is two of them.
    along, up = 2*STAIR_TREAD/max(.01, -climb.y), 2*STAIR_RISE/max(.01, climb.z)
    targets = []
    for f in range(frames+1):
        targets.append({})
        for s in "lr":
            mean = sum((feet[g][s][0] for g in range(frames)), Vector())/frames
            p = feet[f][s][0]
            targets[f][s] = (Vector((p.x, mean.y+(p.y-mean.y)*along, mean.z+(p.z-mean.z)*up)), feet[f][s][1])
    # The capture's cycle doesn't quite close; spread what's left over across
    # it, so the end meets the start.
    for s in "lr":
        gap = targets[0][s][0]-targets[-1][s][0]
        for f in range(frames+1):
            p, turn = targets[f][s]
            targets[f][s] = (p+gap*(f/frames), turn)
    for b in rig.pose.bones:
        q, loc, scale = base[b.name]
        b.rotation_quaternion = q.copy(); b.location = loc.copy(); b.scale = scale.copy()
    bpy.context.view_layer.update()
    return poses, sway, targets


def author(rig, suffix="", mixamo=MIXAMO, fps=24):
    scene = bpy.context.scene
    scene.render.fps = 30; scene.render.fps_base = 1
    rig.animation_data_create()
    idle = bpy.data.actions.get("Breathing Idle")
    if idle is not None:
        rig.animation_data.action = idle
        if idle.slots: rig.animation_data.action_slot = idle.slots[0]
        scene.frame_set(int(idle.frame_range[0]))
    bpy.context.view_layer.update()
    rest = {b.name:(b.head.copy(), b.matrix.to_quaternion()) for b in rig.pose.bones}
    ladder_stance(rig, rest)
    base = {b.name: (b.rotation_quaternion.copy(), b.location.copy(), b.scale.copy()) for b in rig.pose.bones}
    rig.animation_data.action = None
    kick = wall_kick(rig, mixamo)
    steps = round(DURATIONS["WallLand"]*30)
    landing = capture(rig, mixamo, LANDING, [LANDING_FRAMES[0]+(LANDING_FRAMES[1]-LANDING_FRAMES[0])*f/steps for f in range(steps+1)], Quaternion())
    vaults = {name: capture(rig, mixamo, VAULT, [vault_frame(name, f/round(DURATIONS[name]*30)) for f in range(round(DURATIONS[name]*30)+1)], Quaternion())
              for name in VAULT_KEYS}
    hang = pose_at(rig, *FALL_POSE, fps)
    flipped = mirrored(rig, hang[0])
    for n, q in hang[0].items():
        if n.rsplit("_", 1)[0] in LEGS:
            hang[0][n] = q.slerp(flipped[n], .5)
    steps = round(DURATIONS["Slide"]*30)
    slide = capture(rig, mixamo, SLIDE, [SLIDE_FRAMES[0]+(SLIDE_FRAMES[1]-SLIDE_FRAMES[0])*f/steps for f in range(steps+1)], Quaternion())
    stairs = stairs_cycle(rig, mixamo, base, rest)
    moves = ladder_moves(rig, mixamo)
    frames = round(DURATIONS["LadderExit"]*30)
    # Its last sample is where the hop begins.
    hop = capture(rig, mixamo, VAULT, [hop_frame(f/frames) for f in range(frames+1)]+[LADDER_EXIT_FRAMES[0]], Quaternion())
    for name, duration in DURATIONS.items():
        action = bpy.data.actions.new("Traversal_"+name+suffix)
        action.use_fake_user = True
        rig.animation_data.action = action
        frames = round(duration*30)
        for f in range(frames+1):
            angles, lower, back = pose(name,f/frames)
            for b in rig.pose.bones:
                b.rotation_mode="QUATERNION"
                q, loc, scale = base[b.name]
                axis = b.bone.matrix_local.to_quaternion().inverted() @ Vector((1,0,0))
                b.rotation_quaternion = Quaternion(axis, math.radians(angles.get(b.name,0))) @ q
                b.location = loc.copy()
                b.scale = scale.copy()
                if b.name=="pelvis":
                    b.location += b.bone.matrix_local.to_quaternion().inverted() @ Vector((0,back,LADDER_HIGH if name in ("Ladder", "LadderExit") else lower*rig.data.bones["pelvis"].head_local.z/.975))
            bpy.context.view_layer.update()
            if name.startswith("WallKick"):
                kicked = falling(rig, kick[-1], f/frames) if "Fall" in name else kick[f]
                for n, q in (mirrored(rig, kicked) if name.endswith("Right") else kicked).items():
                    orient(rig.pose.bones[n], q)
            if name == "WallLand":
                poses, hips, metre = landing
                for n, q in poses[f].items():
                    orient(rig.pose.bones[n], q)
                # The hips sink as the capture's do, from where they end up: standing.
                pelvis = rig.pose.bones["pelvis"]
                pelvis.location += pelvis.bone.matrix_local.to_quaternion().inverted() @ Vector((0,0,rest["pelvis"][0].z+hips[f].z-hips[-1].z-pelvis.head.z))
                bpy.context.view_layer.update()
            if name == "Fall":
                hanging, hips = hang
                for n, q in falling(rig, hanging, f/frames, LEG_SWING).items():
                    orient(rig.pose.bones[n], q)
                pelvis = rig.pose.bones["pelvis"]
                pelvis.location += pelvis.bone.matrix_local.to_quaternion().inverted() @ (hips-pelvis.head)
                bpy.context.view_layer.update()
            if name == "StairsUp":
                poses, sway, targets = stairs
                for n, q in poses[f].items():
                    orient(rig.pose.bones[n], q)
                pelvis = rig.pose.bones["pelvis"]
                pelvis.location += pelvis.bone.matrix_local.to_quaternion().inverted() @ (rest["pelvis"][0]+sway[f]-pelvis.head)
                bpy.context.view_layer.update()
                for s, sign in (("l", 1), ("r", -1)):
                    target, turn = targets[f][s]
                    limb(rig, "thigh_"+s, "calf_"+s, "foot_"+s, target, (sign*.35, -1, .45), turn)
            if name == "Slide":
                poses, hips, metre = slide
                for n, q in poses[f].items():
                    orient(rig.pose.bones[n], q)
                # The hips drop and rise as the capture's do, kept over the
                # controller.
                pelvis = rig.pose.bones["pelvis"]
                pelvis.location += pelvis.bone.matrix_local.to_quaternion().inverted() @ Vector((0,rest["pelvis"][0].y-pelvis.head.y,hips[f].z-pelvis.head.z))
                bpy.context.view_layer.update()
            if name in vaults:
                poses, hips, metre = vaults[name]
                for n, q in poses[f].items():
                    orient(rig.pose.bones[n], q)
                up, across = carried(name, f/frames)
                height = hips[f].z-up*(VAULT_BOX*metre+VAULT_HOVER)
                ahead = hips[f].y-hips[0].y-across*(hips[-1].y-hips[0].y)
                if name == "Mantle":
                    stand = smooth(0, 1, (f/frames-MANTLE_STAND)/(1-MANTLE_STAND))
                    for n in poses[f]:
                        b = rig.pose.bones[n]
                        b.rotation_quaternion = b.rotation_quaternion.slerp(base[n][0], stand)
                    height += (rest["pelvis"][0].z-height)*stand
                    ahead *= 1-stand
                    bpy.context.view_layer.update()
                pelvis = rig.pose.bones["pelvis"]
                pelvis.location += pelvis.bone.matrix_local.to_quaternion().inverted() @ Vector((0,rest["pelvis"][0].y+ahead-pelvis.head.y,height-pelvis.head.z))
                bpy.context.view_layer.update()
            if name == "Ladder":
                climb(rig, moves, f/frames, LADDER_HIPS)
            if name == "LadderExit":
                # It climbs on as it was, settling as the hands let go.
                t = f/frames
                rising = min(1, t/LADDER_EXIT_CLIMB)
                climb(rig, moves, (.5+LADDER_EXIT_RISE/(2*RUNG)*rising)%1, LADDER_HIPS*(1-smooth(0, 1, rising)))
            contacts(rig,name,f/frames,rest)
            if name == "LadderExit":
                # Off the rails into the hop, and out of that to standing.
                t = f/frames
                go = smooth(0, 1, (t/LADDER_EXIT_CLIMB-.5)/.5)
                stand = smooth(0, 1, (t-LADDER_EXIT_UP)/(1-LADDER_EXIT_UP))
                up = max(0, min(1, (t-LADDER_EXIT_CLIMB)/(LADDER_EXIT_UP-LADDER_EXIT_CLIMB)))
                across = max(0, min(1, (t-LADDER_EXIT_UP)/(LADDER_EXIT_CROSS-LADDER_EXIT_UP)))
                poses, hips, metre = hop
                pelvis = rig.pose.bones["pelvis"]
                climbing = {b.name: b.rotation_quaternion.copy() for b in rig.pose.bones}
                held = pelvis.location.copy()
                for b in rig.pose.bones:
                    b.rotation_quaternion = base[b.name][0].copy()
                pelvis.location = base["pelvis"][1].copy()
                bpy.context.view_layer.update()
                for n, q in poses[f].items():
                    orient(rig.pose.bones[n], q)
                # Where the hips are from where the hop begins, ending stood
                # on the landing; less how far the controller has come.
                box = VAULT_BOX*metre+.02
                height = hips[f].z+(box+rest["pelvis"][0].z-hips[f].z)*stand
                height -= up*box-(1-min(1, t/LADDER_EXIT_CLIMB))*LADDER_EXIT_RISE
                ahead = hips[f].y-hips[-1].y-LADDER_EXIT_LEAN
                ahead += (across-stand)*LADDER_EXIT_ACROSS-ahead*stand
                pelvis.location += pelvis.bone.matrix_local.to_quaternion().inverted() @ Vector((0,rest["pelvis"][0].y+ahead-pelvis.head.y,height-pelvis.head.z))
                for b in rig.pose.bones:
                    b.rotation_quaternion = climbing[b.name].slerp(b.rotation_quaternion, go).slerp(base[b.name][0], stand)
                pelvis.location = held.lerp(pelvis.location, go)
                bpy.context.view_layer.update()
            for b in rig.pose.bones:
                b.keyframe_insert("rotation_quaternion", frame=f+1)
                if b.name=="pelvis": b.keyframe_insert("location",frame=f+1)
        # Linear interpolation at 30 Hz avoids cubic overshoot at contacts.
        if action.slots:
            bag=action.layers[0].strips[0].channelbag(action.slots[0])
            for curve in bag.fcurves:
                for key in curve.keyframe_points: key.interpolation='LINEAR'
        rig.animation_data.action=None
    for b in rig.pose.bones:
        b.rotation_quaternion=Quaternion(); b.location=(0,0,0)


def authored(name):
    """Whether name is one of the clips this script makes."""
    return name.startswith('Traversal_') or name in MIRRORED.values()


def main():
    args = sys.argv[sys.argv.index("--")+1:]
    folder = os.path.abspath(args[0])
    mixamo = os.path.abspath(args[1]) if len(args) > 1 else MIXAMO
    out = os.path.join(os.path.dirname(os.path.dirname(folder)),"build","traversal")
    os.makedirs(out,exist_ok=True)
    for name in ("man","woman"):
        bpy.ops.wm.read_factory_settings(use_empty=True)
        bpy.ops.import_scene.gltf(filepath=os.path.join(folder,name+".glb"))
        rig=next(o for o in bpy.data.objects if o.type=='ARMATURE')
        fps=bpy.context.scene.render.fps/bpy.context.scene.render.fps_base
        for action in list(bpy.data.actions):
            if authored(action.name): bpy.data.actions.remove(action)
        author(rig, mixamo=mixamo, fps=fps)
        for source, mirror in MIRRORED.items():
            mirror_clip(rig, source, mirror, fps)
        for action in list(bpy.data.actions):
            if not authored(action.name): bpy.data.actions.remove(action)
        bpy.ops.export_scene.gltf(filepath=os.path.join(out,name+"-traversal.glb"),export_format='GLB',export_animations=True,export_animation_mode='ACTIONS',export_morph=False)

if __name__=='__main__': main()
