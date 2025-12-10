use log::error;
use redis::{AsyncCommands, Client};

use crate::error::{Result, ServiceError};
use crate::order_book::OrderBook;

const ORDER_BOOK_TTL: u64 = 60;

pub struct Redis {
    client: Client,
}

impl Redis {
    pub fn new(config: String) -> Result<Self> {
        let client = Client::open(config)?;

        Ok(Redis { client })
    }

    fn build_redis_key(&self, symbol: &str) -> String {
        format!("orderbook-{}", symbol)
    }

    pub async fn get_order_book(&self, symbol: &str) -> Result<Option<OrderBook>> {
        let key = self.build_redis_key(symbol);
        let mut conn = self.client.get_multiplexed_async_connection().await?;

        let result: Option<String> = conn.get(key).await?;

        match result {
            Some(book) => serde_json::from_str::<OrderBook>(&book)
                .map_err(|e| {
                    error!("Failed to deserialize redis data: {}", e);
                    ServiceError::from(e)
                })
                .map(|book| Some(book)),
            None => Ok(None),
        }
    }

    pub async fn add_order_book(&self, symbol: &str, book: &OrderBook) -> Result<()> {
        let key = self.build_redis_key(symbol);
        let mut conn = self.client.get_multiplexed_async_connection().await?;

        let book = serde_json::to_string(book).unwrap();
        let _: () = conn.set_ex(key, book, ORDER_BOOK_TTL).await?;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[ignore]
    #[tokio::test]
    async fn test_add_and_get_order_book() {
        // docker compose up -d redis-test
        let redis = Redis::new("redis://localhost:6379".to_string()).unwrap();

        let order_book = OrderBook::default();
        let result = redis.add_order_book("SOLUSDT", &order_book).await;
        assert!(result.is_ok());

        let result = redis.get_order_book("SOLUSDT").await.unwrap().unwrap();
        assert_eq!(result, order_book);

        let result = redis.get_order_book("BTCETH").await.unwrap();
        assert!(result.is_none());
    }
}
