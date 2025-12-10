use reqwest::{Client, RequestBuilder};
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::error::{Result, ServiceError};
use crate::order_book::{OrderBook, OrderBookEntity};

const EXCHANGE_INFO_URL: &str = "https://api.binance.com/api/v3/exchangeInfo";
const ORDER_BOOK_URL: &str = "https://api.binance.com/api/v3/depth";
const LAST_PRICES_URL: &str = "https://api.binance.com/api/v3/ticker/price";
const ORDER_BOOK_DEPTH: &str = "5000"; // maximum available depth

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

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum BinanceResponseWrapper<T> {
    Ok(T),
    Err(BinanceError),
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
    #[allow(unused)]
    pub symbol: String,
    pub price: Decimal,
}

#[derive(Debug, Deserialize)]
struct BinanceOrderBookResponse {
    bids: Vec<(Decimal, Decimal)>, // (price, qty)
    asks: Vec<(Decimal, Decimal)>, // (price, qty)
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
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    fn request(&self, url: &str) -> RequestBuilder {
        self.client.get(url)
    }

    fn request_with_params(&self, url: &str, params: &[(&str, &str)]) -> RequestBuilder {
        self.request(url).query(params)
    }

    async fn send_request<T: for<'a> Deserialize<'a>>(&self, request: RequestBuilder) -> Result<T> {
        request
            .send()
            .await?
            .json::<T>()
            .await
            .map_err(ServiceError::from)
    }

    pub async fn get_last_price(&self, symbol: &str) -> Result<BinancePriceResponse> {
        let req = self.request_with_params(LAST_PRICES_URL, &[("symbol", symbol)]);
        self.send_request::<BinanceResponse<BinancePriceResponse>>(req)
            .await?
            .into_result()
    }

    pub async fn get_order_book(&self, symbol: &str) -> Result<OrderBook> {
        let params = &[("symbol", symbol), ("limit", ORDER_BOOK_DEPTH)];
        let req = self.request_with_params(ORDER_BOOK_URL, params);

        let resp = self
            .send_request::<BinanceResponse<BinanceOrderBookResponse>>(req)
            .await?
            .into_result()?;

        Ok(resp.into())
    }

    pub async fn get_exchange_info(&self) -> Result<Vec<BinanceExchangeSymbol>> {
        let req = self.request(EXCHANGE_INFO_URL);

        let resp = self
            .send_request::<BinanceResponseWrapper<BinanceExchangeInfoResponse>>(req)
            .await?
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
        let binance = Binance::new();
        let result = binance.get_last_price("SOLUSDT").await;
        println!("Result: {:?}", result);
    }

    #[ignore]
    #[tokio::test]
    async fn test_get_order_book() {
        let binance = Binance::new();
        let result = binance.get_order_book("SOLUSDT").await;
        println!("Result: {:?}", result);
    }

    #[ignore]
    #[tokio::test]
    async fn test_get_exchange_info() {
        let binance = Binance::new();
        let result = binance.get_exchange_info().await;
        println!("Result: {:?}", result);
    }
}
