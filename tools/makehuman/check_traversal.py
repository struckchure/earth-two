"""Verify exported traversal contacts, loop seams and body proportions in Blender.

blender --background --factory-startup --python tools/makehuman/check_traversal.py -- assets/characters
"""
import os
import sys
import bpy


def sample(rig, action, phase):
    rig.animation_data.action = action
    if action.slots:
        rig.animation_data.action_slot = action.slots[0]
    frame = action.frame_range[0] + phase * (action.frame_range[1]-action.frame_range[0])
    bpy.context.scene.frame_set(int(frame), subframe=frame-int(frame))
    bpy.context.view_layer.update()
    return {b.name: (b.head.copy(), b.matrix.to_quaternion()) for b in rig.pose.bones}


def check(folder):
    for body in ("man", "woman"):
        bpy.ops.wm.read_factory_settings(use_empty=True)
        bpy.ops.import_scene.gltf(filepath=os.path.join(folder, body+".glb"))
        rig = next(o for o in bpy.data.objects if o.type == 'ARMATURE')
        ladder = bpy.data.actions['Traversal_Ladder']
        first, last = sample(rig, ladder, 0), sample(rig, ladder, 1)
        for name, (position, rotation) in first.items():
            assert (position-last[name][0]).length < .003, (body, name, "loop position seam")
            assert abs(rotation.dot(last[name][1])) > .999, (body, name, "loop rotation seam")
        hand_heights = {'l':[], 'r':[]}
        planted_heights = {'l':[], 'r':[]}
        for phase in (.10, .20, .30, .40, .60, .70, .80, .90):
            points = sample(rig, ladder, phase)
            foot = 'r' if phase < .5 else 'l'
            for name, sole in [('foot_'+foot, .07)]:
                position = points[name][0]
                height = position.z + .6*phase - sole
                nearest = .09 + round((height-.09)/.3)*.3
                assert abs(height-nearest) < .008, (body, name, phase, "rung drift", height, nearest)
                assert abs(position.y+.285) < .008, (body, name, "foot contact depth")
            for side, sign in (('l',1), ('r',-1)):
                wrist = points['hand_'+side][0]
                palm = points['middle_01_'+side][0]
                assert abs(palm.x-sign*.292) < .004, (body, side, "palm left rail", palm)
                assert abs(palm.y+.38) < .004, (body, side, "palm depth", palm)
                hand_heights[side].append(palm.z)
                if side == ('l' if phase < .5 else 'r'):
                    planted_heights[side].append(palm.z+.6*phase)
                # These fail for the old hanging wrist, even when its position is correct.
                assert palm.y < wrist.y-.07 and abs(palm.z-wrist.z) < .004, (body, side, "hanging palm")
                assert points['index_01_'+side][0].z > points['pinky_01_'+side][0].z+.035, (body, side, "thumb is not up")
                thumb = rig.pose.bones['thumb_03_'+side].tail
                finger = rig.pose.bones['middle_03_'+side].tail
                assert thumb.y > -.37 and finger.y < -.39, (body, side, "thumb does not oppose fingers")
                # Rail is a 7 cm square, not merely a wrist target. Joint centres
                # and phalanges must wrap outside its section, never through it.
                for digit in ('index','middle','ring','pinky','thumb'):
                    for joint in (1,2,3):
                        bone = rig.pose.bones[f'{digit}_{joint:02d}_{side}']
                        for fraction in (0,.25,.5,.75,1):
                            point = bone.head.lerp(bone.tail,fraction)
                            outside = max(abs(sign*point.x-.24)-.035,abs(point.y+.37)-.035)
                            assert outside > -.002, (body, bone.name, "finger penetrates rail", point)
        for side in ('l','r'):
            assert max(hand_heights[side])-min(hand_heights[side]) > .20, (body,side,"hand does not travel along rail")
            assert max(planted_heights[side])-min(planted_heights[side]) < .008, (body,side,"supporting palm slides")
        # The left foot taps the wall behind the body, on its left, pushes
        # off it, then comes back under; WallKickRight mirrors it.
        wall = bpy.data.actions['Traversal_WallKick']
        tap, push, away = sample(rig,wall,0), sample(rig,wall,.25), sample(rig,wall,1)
        assert tap['foot_l'][0].x > .3 and tap['foot_l'][0].y > .3, (body,"kicking foot does not reach the wall")
        assert push['foot_l'][0].y > tap['foot_l'][0].y+.08, (body,"missing kick extension")
        for phase in (0,.25,.5,.75):
            assert sample(rig,wall,phase)['foot_r'][0].y < .2, (body,"the other foot goes to the wall too")
        assert away['foot_l'][0].y < .1, (body,"missing kick recovery")
        # The fall carries on from the kick's last pose, moves, and loops.
        fall = bpy.data.actions['Traversal_WallKickFall']
        first, middle, last = sample(rig,fall,0), sample(rig,fall,.4), sample(rig,fall,1)
        for name in ('foot_l','foot_r','hand_l','hand_r'):
            assert (first[name][0]-away[name][0]).length < .08, (body,name,"fall does not start from the kick")
            assert (first[name][0]-last[name][0]).length < .003, (body,name,"fall loop seam")
        assert (first['hand_l'][0]-middle['hand_l'][0]).length > .02, (body,"fall is stiff")
        # Landing sinks into the knees and stands again, feet on the ground.
        land = bpy.data.actions['Traversal_WallLand']
        touch, low, up = sample(rig,land,0), sample(rig,land,.35), sample(rig,land,1)
        assert low['pelvis'][0].z < touch['pelvis'][0].z-.15 and low['pelvis'][0].z < up['pelvis'][0].z-.2, (body,"landing does not sink")
        for pose in (touch, low, up):
            for side in ('l','r'):
                # By the ball of the foot: the back foot lands on its toes.
                assert abs(pose['ball_'+side][0].z-up['ball_'+side][0].z) < .05, (body,side,"landing foot off the ground")
        mirror = sample(rig,bpy.data.actions['Traversal_WallKickRight'],0)
        for side, other in (('l','r'),('r','l')):
            a, b = tap['foot_'+side][0], mirror['foot_'+other][0]
            assert abs(a.x+b.x) < .01 and abs(a.y-b.y) < .01 and abs(a.z-b.z) < .01, (body,side,"right kick is not the left's mirror")
        # Matrix-based posing used to accumulate scale in finger chains. Verify
        # every exported traversal clip preserves the rig's bind bone lengths.
        for action in bpy.data.actions:
            if not action.name.startswith('Traversal_'):
                continue
            for phase in (0,.25,.5,.75,1):
                sample(rig,action,phase)
                for bone in rig.pose.bones:
                    assert abs(bone.length-bone.bone.length) < .001, (body,action.name,bone.name,"stretched bone")
        slide = bpy.data.actions['Traversal_Slide']
        crouch = bpy.data.actions['Traversal_Crouch']
        recover = bpy.data.actions['Traversal_StandUp']
        for phase in (0, .2, .4, .6, .8, 1):
            points = sample(rig, slide, phase)
            for side in ('l','r'):
                assert abs(points['foot_'+side][0].z-.085) < .01, (body, "slide foot left ground", phase)
        # Slide -> crouch -> stand-up share the same contact pose.
        end, hold, start = sample(rig, slide, 1), sample(rig, crouch, 0), sample(rig, recover, 0)
        for name in end:
            assert (end[name][0]-hold[name][0]).length < .003, (body, name, "slide recovery seam")
            assert (hold[name][0]-start[name][0]).length < .003, (body, name, "standing recovery seam")
        print(body, ': side-rail grips, planted rung feet, slide feet, wall kicks, loop and recovery seams passed')


if __name__ == '__main__':
    check(os.path.abspath(sys.argv[sys.argv.index('--')+1]))
