## intro comp -> instrument

### keyframe 1

given the current layout all keys by `(k, g)` are same radius and same position as the sun (see commented out default for exact matches, with the view_port).

all keys are aligned with the sun.

all bands are same radius and are aligned with the sun.

the strings are scaled and rotated to match with flute lines of the composition

### keyframe 1.5

the picture fades

the bands are getting large varying radius increments greater than sun

### keyframe 2

viewbox scaled to layout.space (see layout context)

strings rotation to normal

strings scale to normal

position of keys and bands to collapsed on main axis of the insrument (k=0, g=0)

bands radiuses back to match key radiuses

### keyframe 3

buttons, bands and groups move to correct positions

### keyframe 4

bands recover width/height difference

bands recover padding

### the end

animation cleared, intro composition hidden with <Show></Show>


### effectively back: instrument -> intro comp

is building same animation keyframes as forward but then calling AnimationSequence::<..>.reverse() [@sequence.rs (316:317)](file:///Users/anvlkv/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/keyframe-1.1.1/src/sequence.rs#L316:317)



## intro comp -> tuner (imaginary for now)

### keyframe 1

given the current layout all sensors by `(k, g)` are same radius and same position as the sun (see commented out default for exact matches, with the view_port).

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

.reverse()


## instrument -> tuner


### keyframe 1

keys and sensors match (k,g)

splits at 0

lines match with left string

### keyframe 1.5

bands collapse to circles + padding

### keyframe 1.75

bands collapse to circles of key diameter

strings collapse into left string line [@layout.rs (19:20)](file:///Users/anvlkv/Projects/red-siren/shared/src/instrument/layout.rs#L19:20)

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

.reverse()


## effectively cycle: we can transition between all states of the intro composition.
