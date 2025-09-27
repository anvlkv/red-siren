## Enhanced Example with Net Backend Split

```rust
use fundsp::hacker::*;
use std::thread;
use std::time::Duration;

fn main() {
    // Create a snoop node for thread communication
    let (frontend_snoop, backend_snoop) = snoop(1024); // Buffer size of 1024 samples

    // Create a Net and build the audio graph
    let mut net = Net::new(0, 1);

    // Add audio source (440 Hz sine wave)
    let sine_id = net.push(Box::new(sine_hz(440.0)));

    // Add the backend snoop node to the network
    let snoop_id = net.push(Box::new(backend_snoop));

    // Connect sine to snoop
    net.pipe_all(sine_id, snoop_id).unwrap();

    // Connect snoop to output
    net.pipe_output(snoop_id);

    // Set sample rate
    net.set_sample_rate(44100.0);

    // Split the Net into backend for real-time processing
    let mut backend = net.backend();

    // Thread 1: Real-time audio processing backend
    let backend_handle = thread::spawn(move || {
        // Process audio for 5 seconds
        for _ in 0..220500 { // 44100 * 5 samples
            let _output = backend.get_mono();
        }
    });

    // Thread 2: Frontend data reader
    let frontend_handle = thread::spawn(move || {
        // Wait a bit for audio to start
        thread::sleep(Duration::from_millis(100));

        // Read samples from the snoop frontend
        for i in 0..100 {
            let samples = frontend_snoop.read();
            println!("Iteration {}: Read {} samples", i, samples.len());
            thread::sleep(Duration::from_millis(50));
        }
    });

    // Thread 3: Frontend control thread (optional)
    let control_handle = thread::spawn(move || {
        thread::sleep(Duration::from_millis(1000));

        // Make changes to the frontend Net
        net.replace(sine_id, Box::new(sine_hz(880.0))).unwrap();
        net.commit(); // Apply changes to backend

        println!("Changed frequency to 880 Hz");

        thread::sleep(Duration::from_millis(2000));

        // Smooth crossfade to a different sound
        net.crossfade(sine_id, Fade::Smooth, 0.5, Box::new(saw_hz(220.0)));
        net.commit();

        println!("Crossfaded to 220 Hz saw wave");
    });

    // Wait for all threads to complete
    backend_handle.join().unwrap();
    frontend_handle.join().unwrap();
    control_handle.join().unwrap();
}
```

## Key Components Explained

**Net Backend Split**: The `net.backend()` method creates a real-time safe backend that can be moved to an audio processing thread. The original `net` becomes the frontend for making changes.[1]

**Frontend Control**: The frontend `net` can make changes like:
- `replace()`: Replace a node with another
- `crossfade()`: Smoothly transition between nodes to avoid clicks
- `commit()`: Apply frontend changes to the backend

**Thread Safety**:
- The **backend** handles real-time audio processing safely
- The **snoop frontend** allows safe reading of processed samples
- The **net frontend** enables safe modification of the audio graph structure

## Benefits of This Architecture

1. **Real-time Safety**: The backend processes audio without allocations or blocking operations
2. **Dynamic Control**: The frontend can modify the audio graph while audio continues playing
3. **Sample Access**: The snoop system provides thread-safe access to processed audio data
4. **Smooth Transitions**: Crossfading prevents audio artifacts when changing the graph

This pattern is ideal for interactive audio applications where you need both real-time audio processing and the ability to modify the audio graph dynamically based on user input or other events.
