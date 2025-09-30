use reqwest::Client;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::error::{Result, ServiceError};

const EXCHANGE_INFO_URL: &str = "https://api.binance.com/api/v3/exchangeInfo";
const ORDER_BOOK_URL: &str = "https://api.binance.com/api/v3/depth";
const LAST_PRICES_URL: &str = "https://api.binance.com/api/v3/ticker/price";
const ORDER_BOOK_DEPTH: &str = "5000"; // maximum available depth

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum BinanceResponseWrapper<T> {
    Ok(T),
    Err(BinanceError),
}

#[allow(unused)]
#[derive(Debug, Deserialize)]
pub struct BinanceError {
    #[serde(alias = "code")]
    pub status: i32,
    #[serde(alias = "msg")]
    pub message: String,
}

impl From<BinanceError> for crate::error::ServiceError {
    fn from(value: BinanceError) -> Self {
        Self::Internal(value.message)
    }
}

impl<T> BinanceResponseWrapper<T> {
    fn into_result(self) -> Result<T> {
        match self {
            BinanceResponseWrapper::Ok(value) => Ok(value),
            BinanceResponseWrapper::Err(e) => Err(ServiceError::from(e)),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum BinanceResponse<T> {
    Ok(T),
    None,
}

impl<T> BinanceResponse<T> {
    fn into_result(self) -> Result<T> {
        match self {
            BinanceResponse::Ok(value) => Ok(value),
            BinanceResponse::None => Err(ServiceError::internal(
                "Binance returned no data".to_string(),
            )),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct BinancePriceResponse {
    pub symbol: String,
    pub price: Decimal,
}

#[derive(Debug, Deserialize)]
struct BinanceOrderBookResponse {
    bids: Vec<(Decimal, Decimal)>, // (price, qty)
    asks: Vec<(Decimal, Decimal)>, // (price, qty)
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct OrderBookEntity {
    pub price: Decimal,
    pub qty: Decimal,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct OrderBook {
    pub asks: Vec<OrderBookEntity>,
    pub bids: Vec<OrderBookEntity>,
}

impl From<BinanceOrderBookResponse> for OrderBook {
    fn from(value: BinanceOrderBookResponse) -> Self {
        let to_entity = |entity: Vec<(Decimal, Decimal)>| -> Vec<OrderBookEntity> {
            entity
                .into_iter()
                .map(|(price, qty)| OrderBookEntity { price, qty })
                .collect()
        };

        let asks = to_entity(value.asks);
        let bids = to_entity(value.bids);

        OrderBook { asks, bids }
    }
}

#[cfg(test)]
impl OrderBook {
    pub fn asks() -> Vec<OrderBookEntity> {
        vec![
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
            OrderBookEntity {
                price: Decimal::from(250),
                qty: Decimal::ONE,
            },
        ]
    }

    pub fn bids() -> Vec<OrderBookEntity> {
        vec![
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
        ]
    }

    pub fn default() -> OrderBook {
        OrderBook {
            asks: OrderBook::asks(),
            bids: OrderBook::bids(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct BinanceExchangeSymbol {
    pub symbol: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]

struct BinanceExchangeInfoResponse {
    symbols: Vec<BinanceExchangeSymbol>,
}

pub struct Binance {
    client: Client,
}

impl Binance {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub async fn get_last_price(&self, symbol: &str) -> Result<BinancePriceResponse> {
        self.client
            .get(LAST_PRICES_URL)
            .query(&[("symbol", symbol)])
            .send()
            .await?
            .json::<BinanceResponse<BinancePriceResponse>>()
            .await
            .map_err(ServiceError::from)?
            .into_result()
    }

    pub async fn get_order_book(&self, symbol: &str) -> Result<OrderBook> {
        let resp = self
            .client
            .get(ORDER_BOOK_URL)
            .query(&[("symbol", symbol), ("limit", ORDER_BOOK_DEPTH)])
            .send()
            .await?
            .json::<BinanceResponse<BinanceOrderBookResponse>>()
            .await
            .map_err(ServiceError::from)?
            .into_result()?;

        Ok(resp.into())
    }

    pub async fn get_exchange_info(&self) -> Result<Vec<BinanceExchangeSymbol>> {
        let resp = self
            .client
            .get(EXCHANGE_INFO_URL)
            .send()
            .await?
            .json::<BinanceResponseWrapper<BinanceExchangeInfoResponse>>()
            .await
            .map_err(ServiceError::from)?
            .into_result()?
            .symbols;

        Ok(resp)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[ignore]
    #[tokio::test]
    async fn test_get_last_price() {
        let binance = Binance::new(Client::new());
        let result = binance.get_last_price("SOLUSDT").await;
        print!("Result: {:?}", result);
    }

    #[ignore]
    #[tokio::test]
    async fn test_get_order_book() {
        let binance = Binance::new(Client::new());
        let result = binance.get_order_book("SOLUSDT").await;
        print!("Result: {:?}", result);
    }

    #[ignore]
    #[tokio::test]
    async fn test_get_exchange_info() {
        let binance = Binance::new(Client::new());
        let result = binance.get_exchange_info().await;
        print!("Result: {:?}", result);
    }
}
