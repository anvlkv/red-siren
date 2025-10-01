# Best State Management Alternatives to Tokio Sync Mutex in Tauri v2

When working with Tauri v2, **tokio::sync::Mutex** can indeed lead to deadlocks, especially when held across `.await` points or in complex async scenarios. Here are the most effective alternatives for app state management:

## Recommended Alternatives

### 1. **parking_lot::Mutex** - Best Overall Choice

The most widely recommended alternative for short-lived locks where you don't need to hold the mutex across `.await` points:[1][2]

```rust
use parking_lot::Mutex;
use std::sync::Arc;

struct AppState {
    counter: Arc<Mutex<i32>>,
    data: Arc<Mutex<HashMap<String, String>>>,
}

#[tauri::command]
fn update_counter(state: tauri::State<AppState>) -> i32 {
    let mut counter = state.counter.lock();
    *counter += 1;
    *counter
}
```

**Advantages:**
- Much faster than tokio::sync::Mutex[2]
- Cannot deadlock like std::sync::Mutex in async contexts[1]
- No need for `.await` on lock acquisition
- Excellent for data that doesn't require async operations while locked

### 2. **RwLock for Read-Heavy Workloads**

If your state is mostly read with occasional writes, `parking_lot::RwLock` or `tokio::sync::RwLock` can be more efficient:[3][1]

```rust
use parking_lot::RwLock;
use std::sync::Arc;

struct AppState {
    config: Arc<RwLock<Config>>,
}

#[tauri::command]
async fn get_config(state: tauri::State<'_, AppState>) -> Config {
    let config = state.config.read();
    config.clone()
}

#[tauri::command]
async fn update_config(state: tauri::State<'_, AppState>, new_config: Config) {
    let mut config = state.config.write();
    *config = new_config;
}
```

### 3. **Channel-Based Actor Pattern**

For complex state management with heavy async operations, use channels to create an actor pattern:[4]

```rust
use tokio::sync::mpsc;

enum StateMessage {
    GetCounter { response: oneshot::Sender<i32> },
    Increment { response: oneshot::Sender<i32> },
}

struct StateActor {
    counter: i32,
    receiver: mpsc::Receiver<StateMessage>,
}

impl StateActor {
    async fn run(&mut self) {
        while let Some(msg) = self.receiver.recv().await {
            match msg {
                StateMessage::GetCounter { response } => {
                    let _ = response.send(self.counter);
                }
                StateMessage::Increment { response } => {
                    self.counter += 1;
                    let _ = response.send(self.counter);
                }
            }
        }
    }
}

#[tauri::command]
async fn increment_counter(
    state: tauri::State<'_, mpsc::Sender<StateMessage>>
) -> Result<i32, String> {
    let (tx, rx) = oneshot::channel();
    state.send(StateMessage::Increment { response: tx })
        .await
        .map_err(|_| "Actor unavailable")?;
    rx.await.map_err(|_| "Response failed")
}
```

### 4. **Arc<Mutex<T>> Pattern for Shared State**

When you need to share state across threads or tasks:[5]

```rust
use std::sync::Arc;
use parking_lot::Mutex;

struct AppState {
    shared_data: Arc<Mutex<HashMap<String, String>>>,
}

#[tauri::command]
async fn background_task(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let data = Arc::clone(&state.shared_data);

    tauri::async_runtime::spawn(async move {
        // Safe to move the Arc into the spawned task
        let mut map = data.lock();
        map.insert("key".to_string(), "value".to_string());
    });

    Ok(())
}
```

## State Management Best Practices for Tauri v2

### Setup in main.rs

```rust
fn main() {
    let state = AppState {
        counter: Arc::new(Mutex::new(0)),
        config: Arc::new(RwLock::new(Config::default())),
    };

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            get_counter,
            increment_counter,
            get_config
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### Accessing AppHandle in Background Tasks

When you need to access state from background threads:[6]

```rust
#[tauri::command]
async fn start_background_task(app: tauri::AppHandle) -> Result<(), String> {
    tokio::spawn(async move {
        // Use AppHandle to get state on demand
        let state: tauri::State<AppState> = app.state();
        // Work with state...
    });
    Ok(())
}
```

## Key Recommendations

1. **Use `parking_lot::Mutex`** for most synchronous state operations[2]
2. **Use `RwLock`** for read-heavy workloads[1]
3. **Use channel-based actors** for complex async state management[4]
4. **Avoid holding locks across `.await` points**[7][8]
5. **Prefer `Arc<Mutex<T>>`** when sharing state across tasks[5]
6. **Use `AppHandle`** to access state in background threads[6]

The choice depends on your specific use case: if you're doing simple data access without async operations while locked, `parking_lot::Mutex` is your best bet. For read-heavy scenarios, use `RwLock`. For complex async state management, consider the actor pattern with channels.
