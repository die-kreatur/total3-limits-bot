use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

const TOP_LIMITS: usize = 10;

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

#[derive(Debug, Clone, Copy)]
pub enum OrderType {
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

impl ExtendedOrderBook {
    pub fn asks_volume(&self) -> Decimal {
        self.asks.iter().map(|item| item.qty).sum()
    }

    pub fn bids_volume(&self) -> Decimal {
        self.bids.iter().map(|item| item.qty).sum()
    }
}

fn find_border_price(last_price: Decimal, depth: Decimal, order_type: OrderType) -> Decimal {
    let depth = depth / Decimal::ONE_HUNDRED;

    let percentage = match order_type {
        OrderType::Ask => Decimal::ONE + depth,
        OrderType::Bid => Decimal::ONE - depth,
    };

    last_price * percentage
}

fn trim_order_book_entity(
    book: Vec<OrderBookEntity>,
    border_price: Decimal,
    order_type: OrderType,
) -> Vec<OrderBookEntity> {
    book.into_iter()
        .filter(|entry| match order_type {
            OrderType::Ask => entry.price <= border_price,
            OrderType::Bid => entry.price >= border_price,
        })
        .map(|entity| {
            let price = entity.price.trunc_with_scale(5).normalize();
            OrderBookEntity {
                price,
                qty: entity.qty * price
            }
        })
        .collect()
}

fn sort_and_filter(mut book: Vec<OrderBookEntity>) -> Vec<OrderBookEntity> {
    // sorting by quantity from the biggest one to the smallest one
    book.sort_by(|book1, book2| book2.qty.cmp(&book1.qty));
    book.into_iter().take(TOP_LIMITS).collect()
}

pub fn process_order_book_entity(
    book: Vec<OrderBookEntity>,
    last_price: Decimal,
    depth: Decimal,
    order_type: OrderType,
) -> Vec<OrderBookEntity> {
    let border_price = find_border_price(last_price, depth, order_type);
    let entities = trim_order_book_entity(book, border_price, order_type);
    sort_and_filter(entities)
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

        let result = trim_order_book_entity(bids, Decimal::from(83), OrderType::Bid);
        let expected = vec![
            OrderBookEntity {
                price: Decimal::from(90),
                qty: Decimal::from(900),
            },
            OrderBookEntity {
                price: Decimal::from(85),
                qty: Decimal::from(8500),
            },
        ];

        assert_eq!(result, expected);

        let result = trim_order_book_entity(asks, Decimal::from(200), OrderType::Ask);
        let expected = vec![
            OrderBookEntity {
                price: Decimal::ONE_HUNDRED,
                qty: Decimal::ONE_HUNDRED,
            },
            OrderBookEntity {
                price: Decimal::from(150),
                qty: Decimal::from(1500),
            },
            OrderBookEntity {
                price: Decimal::from(200),
                qty: Decimal::from(400),
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
                price: Decimal::from(85),
                qty: Decimal::ONE_HUNDRED,
            },
            OrderBookEntity {
                price: Decimal::from(90),
                qty: Decimal::TEN,
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
