use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use log::{error, info};
use reqwest::Client;
use rust_decimal::Decimal;
use teloxide::types::ChatId;
use tokio::sync::RwLock;
use tokio::time::interval;

use crate::binance::Binance;
use crate::error::{Result, ServiceError};
use crate::order_book::{ExtendedOrderBook, OrderBook, OrderType, process_order_book_entity};
use crate::redis::Redis;

const DEPTH_EXEPCTIONS: [&str; 4] = ["BTCUSDT", "ETHUSDT", "WBTCUSDT", "WETHUSDT"];

pub struct AppState {
    binance: Binance,
    trading_pairs: RwLock<HashSet<String>>,
    redis: Redis,
    allowed_users: HashSet<ChatId>,
}

impl AppState {
    pub fn new(redis_config: String, allowed_users: HashSet<ChatId>) -> Self {
        let redis = Redis::new(redis_config).expect("Failed to connect to Redis");

        AppState {
            binance: Binance::new(Client::new()),
            trading_pairs: RwLock::new(HashSet::new()),
            redis,
            allowed_users,
        }
    }

    async fn get_usdt_trading_pairs(&self) -> Result<Vec<String>> {
        let exch_info = self
            .binance
            .get_exchange_info()
            .await?
            .into_iter()
            .filter(|item| item.status == "TRADING" && item.symbol.ends_with("USDT"))
            .map(|item| item.symbol)
            .collect();

        Ok(exch_info)
    }

    async fn get_order_book(&self, symbol: &str) -> Result<OrderBook> {
        let redis_ob = self.redis.get_order_book(symbol).await?;

        match redis_ob {
            Some(ob) => Ok(ob),
            None => match self.binance.get_order_book(&symbol).await {
                Ok(book) => {
                    let _ = self.redis.add_order_book(symbol, &book).await.map_err(|e| {
                        error!("Failed to save order book for {} due to error: {}", symbol, e);
                    });
                    Ok(book)
                },
                Err(e) => Err(ServiceError::from(e))
            }
        }
    }

    pub async fn get_filtered_order_book(
        &self,
        symbol: String,
        depth: Decimal,
    ) -> Result<ExtendedOrderBook> {
        let last_price = self.binance.get_last_price(&symbol).await?;
        let order_book = self.get_order_book(&symbol).await?;

        let asks = process_order_book_entity(order_book.asks, last_price.price, depth, OrderType::Ask);
        let bids = process_order_book_entity(order_book.bids, last_price.price, depth, OrderType::Bid);

        Ok(ExtendedOrderBook {
            symbol,
            asks,
            bids,
            last_price: last_price.price,
            depth,
        })
    }

    pub async fn validate_symbol(&self, symbol: &str) -> Result<String> {
        let symbol = symbol.to_uppercase();

        let symbol = if !symbol.ends_with("USDT") {
            format!("{}USDT", symbol)
        } else {
            symbol
        };

        if DEPTH_EXEPCTIONS.contains(&symbol.as_str()) {
            return Err(ServiceError::UnsupportedSymbol(symbol));
        }

        let exch_info = self.trading_pairs.read().await;

        match exch_info.get(&symbol) {
            None => Err(ServiceError::SymbolNotFound(symbol)),
            Some(item) => Ok(item.to_owned()),
        }
    }

    pub async fn authorize(&self, chat_id: ChatId) -> Result<()> {
        if !self.allowed_users.contains(&chat_id) {
            return Err(ServiceError::Unauthorized)
        }

        Ok(())
    }
}

pub async fn periodic_exchange_info_update(state: Arc<AppState>) {
    let mut interval = interval(Duration::from_secs(300));
    info!("Updating exchange info Binance Spot");

    loop {
        interval.tick().await;

        match state.get_usdt_trading_pairs().await {
            Err(e) => error!("Failed to update exchange info Binance Futures: {}", e),
            Ok(data) => {
                let mut lock = state.trading_pairs.write().await;
                lock.extend(data);
            }
        }
    }
}
