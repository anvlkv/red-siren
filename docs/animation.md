## intro comp. (load -> home)

no keyframes animation. composition visible as is.

## intro comp -> instrument

### keyframe 1

scales of keyboard and strings computed to match viewBox'es of artwowrk composition (the scale is computed as `key` radius to `#sun` radius)

given the current layout all keys by `(g, k)` are same radius and same position as the sun.

all keys are aligned with the sun. all bands are same **radius** and are aligned with the sun.
effectively, all are collapsed in one circle - the sun the instrument keyboard is aligned with the top left of the screen
ie. negative translation applies to stack them all under sun (uses individual translations)

the strings are scaled and rotated 180 + x degrees to match with `#flute` lines of the composition.
the instrument strings component is aligned with the bottom right of the screen


### keyframe 1.5

the picture fades

the bands are getting large varying radius increments greater than sun. effectively making the sun glare

### keyframe 2

scale and translation of instrument parts to normal

strings rotation to normal

position of keys and bands to collapsed on main axis of the insrument (g=max, k=max)

bands radiuses back to match key radiuses (bands are still **circles**)

### keyframe 3

buttons, bands and groups move to correct positions

### keyframe 4

bands recover width/height difference, ¡only now becoming rounded rectangles!

bands recover padding

### the end

animation cleared


### effectively back: instrument -> intro comp

is building same animation keyframes as forward but `animation-direction` reverse.



## intro comp -> tuner (imaginary for now)

### keyframe 1

given the current layout all sensors by `(g, k)` are same radius and same position as the sun (see commented out default for exact matches, with the view_port).

all sensors are aligned with the sun.

all splits are all zeroes.

the lines are scaled and rotated to match with flute lines of the composition

### keyframe 1.5

the picture fades

the splits are getting large varying radius increments greater than sun

### keyframe 2

viewbox scaled to layout.space (see layout context)

lines rotation to normal

lines scale to normal

lines positioned to normal

splits are back at 0

### keyframe 3

sensors are at their normal positions

splits are at their set values


### the end

animation cleared, intro composition hidden


### effectively back: tuner -> intro comp

reverse


## instrument -> tuner


### keyframe 1

keys and sensors match (g,k)

splits at 0

lines match with left string

### keyframe 1.5

bands collapse to circles + padding

### keyframe 1.75

bands collapse to circles of key diameter

all strings collapse into left string line

### keyframe 2

strings and and tuner lines to tuner lines' positions

keys and sensors to sensors positions

instrument fade away

### keyframe 3

splits to tuner data values

sensors to tuner data values

### the end

animation cleared, navigation commited

### effectively back: tuner -> instrument

reverse


## effectively cycle: we can transition between all states of the intro composition.
