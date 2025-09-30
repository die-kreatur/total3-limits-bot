use std::collections::HashSet;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use log::{error, info};
use reqwest::Client;
use rust_decimal::Decimal;
use tokio::sync::RwLock;
use tokio::time::interval;

use crate::binance::{Binance, OrderBook, OrderBookEntity};
use crate::error::{Result, ServiceError};
use crate::redis::Redis;

const DEPTH_EXEPCTIONS: LazyLock<&[&str]> = LazyLock::new(|| &["BTCUSDT", "ETHUSDT", "WBTCUSDT", "WETHUSDT"]);
const TOP_LIMITS: usize = 10;

#[derive(Debug, Clone, Copy)]
enum OrderType {
    Ask,
    Bid,
}

pub struct ExtendedOrderBook {
    pub symbol: String,
    pub asks: Vec<OrderBookEntity>,
    pub bids: Vec<OrderBookEntity>,
    pub last_price: Decimal,
    pub depth: Decimal,
}

fn find_border_price(last_price: Decimal, depth: Decimal, order_type: OrderType) -> Decimal {
    let depth = depth / Decimal::ONE_HUNDRED;

    let percentage = match order_type {
        OrderType::Ask => Decimal::ONE + depth,
        OrderType::Bid => Decimal::ONE - depth,
    };

    last_price * percentage
}

fn trim_order_book(
    book: Vec<OrderBookEntity>,
    border_price: Decimal,
    order_type: OrderType,
) -> Vec<OrderBookEntity> {
    book.into_iter()
        .filter(|entry| match order_type {
            OrderType::Ask => entry.price <= border_price,
            OrderType::Bid => entry.price >= border_price,
        })
        .map(|mut entity| {
            entity.price = entity.price.trunc_with_scale(4).normalize();
            entity
        })
        .collect()
}

fn sort_and_filter(mut book: Vec<OrderBookEntity>) -> Vec<OrderBookEntity> {
    // sorting by quantity from the biggest one to the smallest one
    book.sort_by(|book1, book2| book2.qty.cmp(&book1.qty));
    book.into_iter().take(TOP_LIMITS).collect()
}

fn process_order_book(
    book: Vec<OrderBookEntity>,
    last_price: Decimal,
    depth: Decimal,
    order_type: OrderType,
) -> Vec<OrderBookEntity> {
    let border_price = find_border_price(last_price, depth, order_type);
    let entities = trim_order_book(book, border_price, order_type);
    sort_and_filter(entities)
}

pub struct AppState {
    binance: Binance,
    trading_pairs: RwLock<HashSet<String>>,
    redis: Redis,
}

impl AppState {
    pub fn new(redis_config: String) -> Self {
        let redis = Redis::new(redis_config).expect("Failed to connect to Redis");

        AppState {
            binance: Binance::new(Client::new()),
            trading_pairs: RwLock::new(HashSet::new()),
            redis,
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

        let asks = process_order_book(order_book.asks, last_price.price, depth, OrderType::Ask);
        let bids = process_order_book(order_book.bids, last_price.price, depth, OrderType::Bid);

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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_find_border_price() {
        let last_price = Decimal::from(200);
        let depth = Decimal::TEN;

        let result = find_border_price(last_price, depth, OrderType::Ask);
        let expected = Decimal::from(220);
        assert_eq!(result, expected);

        let result = find_border_price(last_price, depth, OrderType::Bid);
        let expected = Decimal::from(180);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_trim_order_book() {
        let bids = OrderBook::bids();
        let asks = OrderBook::asks();

        let result = trim_order_book(bids, Decimal::from(83), OrderType::Bid);
        let expected = vec![
            OrderBookEntity {
                price: Decimal::from(90),
                qty: Decimal::TEN,
            },
            OrderBookEntity {
                price: Decimal::from(85),
                qty: Decimal::ONE_HUNDRED,
            },
        ];

        assert_eq!(result, expected);

        let result = trim_order_book(asks, Decimal::from(200), OrderType::Ask);
        let expected = vec![
            OrderBookEntity {
                price: Decimal::ONE_HUNDRED,
                qty: Decimal::ONE,
            },
            OrderBookEntity {
                price: Decimal::from(150),
                qty: Decimal::TEN,
            },
            OrderBookEntity {
                price: Decimal::from(200),
                qty: Decimal::TWO,
            },
        ];

        assert_eq!(result, expected);
    }

    #[test]
    fn test_sort_and_filter() {
        let entity = OrderBook::bids();

        let result = sort_and_filter(entity);
        let expected = vec![
            OrderBookEntity {
                price: Decimal::from(90),
                qty: Decimal::TEN,
            },
            OrderBookEntity {
                price: Decimal::from(85),
                qty: Decimal::ONE_HUNDRED,
            },
            OrderBookEntity {
                price: Decimal::from(80),
                qty: Decimal::TWO,
            },
            OrderBookEntity {
                price: Decimal::from(75),
                qty: Decimal::ONE,
            },
        ];

        assert_eq!(result, expected);
    }
}
