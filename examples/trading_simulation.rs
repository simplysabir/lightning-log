use lightning_log::*;
use std::collections::HashMap;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
struct Order {
    id: u64,
    symbol: String,
    side: String, // "BUY" or "SELL"
    quantity: u32,
    price: f64,
    timestamp: u64,
}

#[derive(Debug)]
struct MarketData {
    symbol: String,
    bid_price: f64,
    ask_price: f64,
    bid_size: u32,
    ask_size: u32,
    timestamp: u64,
}

struct TradingEngine {
    orders: HashMap<u64, Order>,
    market_data: HashMap<String, MarketData>,
    next_order_id: u64,
}

impl TradingEngine {
    fn new() -> Self {
        Self {
            orders: HashMap::new(),
            market_data: HashMap::new(),
            next_order_id: 1,
        }
    }

    fn generate_order(&mut self) -> Order {
        let symbols = ["AAPL", "GOOGL", "MSFT", "TSLA", "NVDA"];
        let sides = ["BUY", "SELL"];

        let order = Order {
            id: self.next_order_id,
            symbol: symbols[self.next_order_id as usize % symbols.len()].to_string(),
            side: sides[self.next_order_id as usize % sides.len()].to_string(),
            quantity: 100 + (self.next_order_id % 900) as u32,
            price: 100.0 + (self.next_order_id as f64 % 200.0),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64,
        };

        self.next_order_id += 1;
        order
    }

    fn generate_market_data(&mut self) -> MarketData {
        let symbols = ["AAPL", "GOOGL", "MSFT", "TSLA", "NVDA"];
        let symbol = symbols[self.next_order_id as usize % symbols.len()].to_string();
        let base_price = 150.0 + (self.next_order_id as f64 % 100.0);

        MarketData {
            symbol: symbol.clone(),
            bid_price: base_price - 0.05,
            ask_price: base_price + 0.05,
            bid_size: 1000 + (self.next_order_id % 9000) as u32,
            ask_size: 1000 + (self.next_order_id % 9000) as u32,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64,
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Lightning Log - Trading Simulation Example");
    println!("==========================================");

    // Initialize with high-performance configuration
    let config = LoggerConfig {
        channel_capacity: 1_000_000, // Large buffer for high throughput
        destinations: vec![
            LogDestination::Stdout,
            LogDestination::File(std::path::PathBuf::from("logs/trading_simulation.log")),
        ],
        file_buffer_size: 128 * 1024, // 128KB buffer for file output
        enable_cpu_affinity: true,
    };

    init_lightning_log_with_config(config)?;

    // Register comprehensive message formats for trading
    register_message_format(1001, "ORDER_NEW: id={}, symbol={}, side={}, qty={}, price={:.2}".to_string())?;
    register_message_format(1002, "ORDER_FILLED: id={}, symbol={}, side={}, qty={}, price={:.2}, slippage={:.4}".to_string())?;
    register_message_format(1003, "MARKET_DATA: symbol={}, bid={:.2}, ask={:.2}, bid_size={}, ask_size={}".to_string())?;
    register_message_format(1004, "STRATEGY_SIGNAL: symbol={}, signal={}, confidence={:.2}, timestamp={}".to_string())?;
    register_message_format(1005, "RISK_CHECK: component={}, level={}, threshold={:.2}, value={:.2}".to_string())?;
    register_message_format(1006, "PERF_METRIC: component={}, latency_ns={}, throughput={}, queue_depth={}".to_string())?;

    println!("Trading simulation started. This will generate realistic trading logs...\n");

    let mut engine = TradingEngine::new();
    let start_time = Instant::now();
    let mut message_count = 0;

    // Simulate high-frequency trading activity
    for i in 0..10000 {
        // Generate new order
        let order = engine.generate_order();
        lightning_info!(1001, order.id, &order.symbol, &order.side, order.quantity, order.price);
        message_count += 1;

        // Simulate order fill with some slippage
        if i % 3 == 0 {
            let fill_price = order.price + (rand::random::<f64>() - 0.5) * 0.1;
            let slippage = fill_price - order.price;
            lightning_info!(1002, order.id, &order.symbol, &order.side, order.quantity, fill_price, slippage);
            message_count += 1;
        }

        // Generate market data updates
        let market_data = engine.generate_market_data();
        lightning_info!(1003, &market_data.symbol, market_data.bid_price, market_data.ask_price, market_data.bid_size, market_data.ask_size);
        message_count += 1;

        // Generate strategy signals occasionally
        if i % 10 == 0 {
            let signals = ["BUY", "SELL", "HOLD"];
            let signal = signals[i % signals.len()];
            let confidence = 0.5 + rand::random::<f64>() * 0.5;
            lightning_info!(1004, &market_data.symbol, signal, confidence, market_data.timestamp);
            message_count += 1;
        }

        // Risk checks
        if i % 25 == 0 {
            let risk_level = (i % 100) as f64 / 100.0;
            let threshold = 0.8;
            lightning_info!(1005, "position_risk", risk_level, threshold, risk_level);
            message_count += 1;

            if risk_level > threshold {
                lightning_warn!(2001, "High risk level detected", risk_level);
                message_count += 1;
            }
        }

        // Performance metrics
        if i % 50 == 0 {
            let latency_ns = (i % 1000) as u64 + 100; // Simulated latency
            let throughput = message_count as f64 / start_time.elapsed().as_secs_f64();
            let queue_depth = (i % 100) as u32;
            lightning_info!(1006, "order_processor", latency_ns, throughput as u32, queue_depth);
            message_count += 1;
        }

        // Small delay to simulate realistic timing
        if i % 1000 == 0 {
            thread::sleep(Duration::from_millis(1));
        }
    }

    let elapsed = start_time.elapsed();
    let avg_throughput = message_count as f64 / elapsed.as_secs_f64();

    println!("\nTrading simulation completed!");
    println!("Messages processed: {}", message_count);
    println!("Time elapsed: {:.2}s", elapsed.as_secs_f64());
    println!("Average throughput: {:.0} messages/sec", avg_throughput);
    println!("Logs written to: logs/trading_simulation.log");

    // Graceful shutdown
    shutdown_lightning_log();
    thread::sleep(Duration::from_millis(200)); // Allow time for final log processing

    Ok(())
}
