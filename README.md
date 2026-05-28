# polybot-sdk-v2

Get your Rust GUI running on Windows Server 2019 **without a GPU** by using **Mesa3D software OpenGL**.

---

## Step 1: Download Mesa3D Windows Build

1. Go to this GitHub page:
   [https://github.com/pal1000/mesa-dist-win](https://github.com/pal1000/mesa-dist-win)
2. Scroll to **Releases** and download the latest **`mesa-dist-win-<version>-x64.zip`** (choose x64 if your server is 64-bit, which it almost certainly is).

---

## Step 2: Extract the DLLs

1. Unzip the file anywhere convenient, e.g., `C:\mesa3d`.
2. Inside, you’ll find a folder `bin` with **`opengl32.dll`** (and maybe `glx.dll`, etc.).
3. Copy **`opengl32.dll`** into the **same folder as your Rust executable**.

   * Example:

     ```
     C:\polybot\release\polybot.exe
     C:\polybot\release\opengl32.dll   <-- copy here
     ```
3. Copy all other **`***.dll`** into the **same folder as your Rust executable**. (mostly dependencies)

> Windows will now use Mesa’s software OpenGL instead of the default Microsoft driver.

---

## Step 3: Run your Rust app

* Double-click your `polybot.exe` or run it from the command line.
* You should now bypass the **OpenGL 2.0+ error** and your GUI should start.

---

### ✅ Notes:

* **Performance:** Software OpenGL is slower than hardware GPU. Good for testing, dashboards, or headless servers, but not for heavy rendering.
* **Debugging:** Mesa will print warnings/errors to the console; these are usually safe to ignore.
* **No GPU needed:** This works even if your server has no graphics card installed.

---

### 1️⃣ Mesa ZINK Vulkan warning

```
MESA: error: ZINK: failed to load vulkan-1.dll
```

* **Cause:** Mesa3D tried to use the ZINK driver (OpenGL over Vulkan) but Vulkan isn’t installed on your server.
* **Effect:** Usually harmless. Mesa can fall back to its **classic OpenGL software renderer**, which is what you want.
* **Action:** You can ignore this unless you specifically want Vulkan acceleration. Mesa will still provide OpenGL 2.0+ in software mode.

---

### 2️⃣ Glow context warning

```
Failed to create context using default context attributes ... os error 203
```

* `os error 203` means **“The system could not find the environment option that was entered”**.
* This happens because **some attributes for the OpenGL context are optional**, and Mesa’s software context may not support them all.
* **Effect:** If your app still creates a window and GUI appears, this warning is safe to ignore.

---

---

### wgpu

Config.toml 
eframe = { version = "...", features = ["wgpu"] }

main.rs
    let native_options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 750.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };

---

## Suggested improvements

6. Add proper feed lifecycle ownership (next step upgrade)

Right now UI is deciding when feeds start.

Better model:

UI emits intent:
UiCommand::EnsureFeed { window_ts, slug }
Worker decides:
start if not running
ignore if already running

This removes HashSet from UI completely.

If you want the cleanest version, we can move to this next.

9. If you want the “production-grade upgrade”

Next step (optional but powerful):

I can help you refactor into:

Event-driven architecture:
UiEvent::WindowCreated
UiEvent::WindowClosed
UiEvent::UserAction

→ fed into a single state reducer

and worker becomes a pure command executor

This removes all timing bugs permanently.

---

If you want, next I can:

refactor your PolymarketDashboardApp into a clean state machine
or redesign the worker into a deterministic “feed supervisor”
or fix your market feed handle lifecycle so shutdown is bulletproof

---

## Official SDK (local updates)


1.
Updated File: 
	polybot_sdk_v2::clob::types::response

use serde::de::Deserializer as DeDeserializer;
use std::str::FromStr;

#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize, Builder, PartialEq)]
#[builder(on(String, into))]
pub struct MakerOrder {
    pub order_id: String,
    pub owner: ApiKey,
    pub maker_address: Address,
    pub matched_amount: Decimal,
    pub price: Decimal,
    #[serde(deserialize_with = "deserialize_decimal_or_empty")]
    pub fee_rate_bps: Option<Decimal>,
    pub asset_id: U256,
    pub outcome: String,
    pub side: Side,
}

pub fn deserialize_decimal_or_empty<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<Decimal>, D::Error>
where
    D: DeDeserializer<'de>,
{
    let v = String::deserialize(deserializer)?;

    if v.trim().is_empty() {
        return Ok(None);
    }

    Decimal::from_str(&v)
        .map(Some)
        .map_err(|e| serde::de::Error::custom(e.to_string()))
}

2. Updated File: 
	polybot_sdk_v2::gamma::types::response

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeeSchedule {
    pub exponent: Option<i32>,
    pub rate: Option<Decimal>,
    pub rebate_rate: Option<Decimal>,
    pub taker_only: Option<bool>,
}

#[serde_as]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Market {
    ...
    pub approved: Option<bool>,
    pub cyom: Option<bool>,
    pub fees_enabled: Option<bool>,
    pub fee_schedule: Option<FeeSchedule>,
    pub fee_type: Option<String>,
    pub holding_rewards_enabled: Option<bool>,
    pub neg_risk: Option<bool>,
    ...
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventMetadata {
    pub price_to_beat: Option<Decimal>,
    pub final_price: Option<Decimal>,
}

#[serde_as]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Event {
    ...
    pub automatically_active: Option<bool>,
    pub event_date: Option<NaiveDate>,
    pub event_metadata: Option<EventMetadata>,
    pub start_time: Option<DateTime<Utc>>,
    pub event_week: Option<i32>,
    ...
}



---

## network architecture change


---

# Better Approach for Trading Dashboards

Instead of continuous repainting:

## Repaint only on events

Whenever worker receives websocket data:

```rust
ctx.request_repaint();
```

That gives:

* near-zero idle CPU
* instant updates
* no unnecessary redraws

You already pass `egui::Context` into the worker:

```rust
worker.ctx = cc.egui_ctx.clone();
```

So inside websocket update handlers:

```rust
self.ctx.request_repaint();
```

is the ideal architecture.

---

# Hybrid Strategy (Recommended)

For trading UIs:

## Low idle refresh

```rust
ctx.request_repaint_after(Duration::from_millis(250));
```

(4 FPS idle)

PLUS:

```rust
ctx.request_repaint();
```

on websocket events.

This gives:

* smooth updates
* responsive UI
* very low idle CPU

---

# Another Potential CPU Contributor

This loop:

```rust
while let Ok(update) = self.update_rx.try_recv()
```

can become expensive if websocket traffic is heavy.

If thousands of updates accumulate per frame:

* every frame drains the queue
* reallocations/layouts happen repeatedly
* vectors grow continuously

Still, that generally scales *with traffic*.

Your repaint loop is unconditional, which is why CPU stays high *all the time*.

---

# Most Likely Breakdown

Based on your code:

| Component                    | Likelihood |
| ---------------------------- | ---------- |
| Continuous egui repaint loop | VERY HIGH  |
| Websocket traffic processing | MEDIUM     |
| Logging/tracing              | MEDIUM     |
| Tokio task churn             | LOW-MEDIUM |
| GPU renderer                 | LOW        |
| Channel overhead             | LOW        |

---

# First Fix I Would Try

Replace:

```rust
ctx.request_repaint_after(Duration::from_millis(33));
```

with:

```rust
ctx.request_repaint_after(Duration::from_millis(250));
```

Then add:

```rust
self.ctx.request_repaint();
```

inside worker update events.

That alone may cut CPU usage by 50–80%.


#####################################################################
HERE STARTS OPTIMIZATION STRATEGY FOR DIFFERENT PROBLEM
#####################################################################

### Option B

Add **single unified market state bus (lock-free snapshot fanout)**
→ improves UI + strategy loops significantly

#####################################################################

Good choice. This is one of those changes that quietly removes a lot of future pain—especially in a worker that’s doing polling + WS + UI updates + order state tracking.

Right now you effectively have **multiple competing state writers**:

* `tracked_orders` (Arc<Mutex<HashMap<...>>)
* `tracked_trades`
* `MarketPrices` (ArcSwap, already good)
* polling loops mutating shared maps
* UI commands mutating same maps
* WS feed mutating prices

That works, but it scales poorly because you get:

* lock contention under load
* partial state visibility (half-updated structs)
* “who won last write?” bugs
* harder reasoning under HFT timing pressure

---

# 🧠 Option B: Single Unified Market State Bus (Lock-free snapshot fanout)

## 🎯 Goal

Replace “shared mutable state everywhere” with:

> One immutable snapshot, atomically replaced, read everywhere.

Think:

```
          +------------------+
          | Worker Engine    |
          | (single writer)  |
          +--------+---------+
                   |
                   v
          ArcSwap<GlobalState>
                   |
     +-------------+--------------+
     |             |              |
   UI thread   polling loops   strategy loops
   (read-only)  (read-only)     (read-only)
```

No locks in read paths.

---

# 🧱 Step 1: Define ONE global state

We unify:

* orders
* trades
* market prices
* feed status
* rapid sell state

---

## ✅ Core structure

```rust
use std::sync::Arc;
use arc_swap::ArcSwap;
use dashmap::DashMap;

#[derive(Clone, Debug)]
pub struct GlobalState {
    pub orders: Arc<DashMap<String, TrackedOrder>>,
    pub trades: Arc<DashMap<String, TradeResponse>>,

    pub markets: Arc<DashMap<u64, MarketPrices>>,

    pub feed_status: Arc<DashMap<u64, FeedStatus>>,

    pub last_update_ts: u64,
}
```

---

## Feed status (important for debugging HFT systems)

```rust
#[derive(Clone, Debug)]
pub struct FeedStatus {
    pub connected: bool,
    pub stale: bool,
    pub last_heartbeat: u64,
    pub error: Option<Arc<str>>,
}
```

---

# 🧱 Step 2: Single shared state handle

```rust
pub type SharedState = Arc<ArcSwap<GlobalState>>;
```

Now EVERYTHING reads from:

```rust
state.load()
```

and writes via snapshot replacement.

---

# 🧱 Step 3: Write model (IMPORTANT)

We do NOT mutate global state directly.

Instead:

### Pattern:

1. clone snapshot
2. mutate local copy
3. atomically swap

---

## Helper:

```rust
fn update_state<F>(state: &SharedState, f: F)
where
    F: FnOnce(&mut GlobalState),
{
    let mut new_state = (*state.load()).clone();
    f(&mut new_state);
    new_state.last_update_ts = current_ts();
    state.store(Arc::new(new_state));
}
```

---

# ⚠️ Important improvement (HFT-safe version)

We avoid cloning DashMaps unnecessarily by **keeping them inside Arc**

So updates mutate DashMap directly (safe, concurrent), but snapshot swap is for metadata consistency.

---

# 🧱 Step 4: Replace your current scattered state

## BEFORE (problematic)

* `Arc<Mutex<HashMap<TrackedOrder>>`
* `Arc<Mutex<HashMap<TradeResponse>>`
* `ArcSwap<MarketPrices>`
* multiple independent truths

---

## AFTER (clean)

Everything becomes:

```rust
state.orders.insert(id, order);
state.trades.insert(id, trade);
state.markets.insert(window_ts, price);
```

NO mutexes.

---

# 🧱 Step 5: Convert your worker

## In PolymarketWorker:

Replace:

```rust
market_tasks
tracked_orders
tracked_trades
orders_to_poll
```

with:

```rust
pub state: SharedState,
```

---

# 🧱 Step 6: Example migration (your order update loop)

### BEFORE:

```rust
let mut lock = tracked_orders.lock().await;
lock.insert(order_id, order);
```

---

### AFTER:

```rust
state.load().orders.insert(order_id, order);
```

No await, no lock contention.

---

# 🧱 Step 7: UI becomes trivial

UI now only does:

```rust
let state = worker.state.load();
let orders = &state.orders;
let markets = &state.markets;
```

No async. No mutex. No contention.

---

# 🧱 Step 8: Why this is HUGE for your system

## You eliminate:

### ❌ Lock contention spikes

* polling loops no longer block UI updates

### ❌ inconsistent views

* order + trade + status mismatch disappears

### ❌ race conditions

* “order updated but trade not yet visible”

---

## You gain:

### ✅ deterministic snapshots

UI always sees consistent state

### ✅ HFT-safe read paths

no await, no locks

### ✅ better scaling under load

DashMap is concurrent, ArcSwap is atomic

---

# 🧱 Step 9: Optional upgrade (recommended later)

If you want to go further HFT-grade:

Add:

```rust
pub version: AtomicU64
```

Every update increments version → enables:

* diff detection
* event replay
* debugging latency gaps

---

# 🧠 Critical design insight (important)

You originally had:

> “many sources mutate shared state”

Now you move to:

> “one logical state, many read-only observers”

That is exactly how:

* trading engines
* order routers
* market data buses

are structured in low-latency systems.

---

# 🚀 Next step (recommended)

If you want to continue properly in HFT direction, the next upgrade is:

## 👉 Option C: Event-driven state bus (instead of direct mutation)

That means:

* worker emits events
* single reducer applies state changes
* fully deterministic replay

This is how serious trading systems avoid hidden bugs.

---

If you want, next we can:

* migrate ONE subsystem (orders) into this model first
* or redesign your entire worker loop into event-sourcing style

Just tell me which direction.
