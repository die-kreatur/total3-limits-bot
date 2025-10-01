use numfmt::Formatter;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

use crate::binance::OrderBookEntity;
use crate::state::ExtendedOrderBook;

const MARKDOVWN2_ESCAPE_SYMBOLS: &str = r#"\\[]()~>#\+-={}.!""#;
const MARKDOVWN2_SYMBOLS: &str = r#"*_"#;

fn escape_markdown_v2(text: String) -> String {
    let mut escaped = String::with_capacity(text.len());

    for c in text.chars() {
        if MARKDOVWN2_ESCAPE_SYMBOLS.contains(c) && !MARKDOVWN2_SYMBOLS.contains(c) {
            escaped.push('\\');
        }
        escaped.push(c);
    }

    escaped
}

fn format_num(f: &mut Formatter, num: Decimal) -> String {
    let num = num.to_f64().unwrap();
    let num = f.fmt2(num);
    num.to_owned()
}

fn format_order_book(mut f: &mut Formatter, book: Vec<OrderBookEntity>) -> String {
    let mut book = book.into_iter().enumerate().map(|(index, entity)| match index {
            0 => format!("{}  •  {} 🥇", entity.price, format_num(&mut f, entity.qty)),
            1 => format!("{}  •  {} 🥈", entity.price, format_num(&mut f, entity.qty)),
            2 => format!("{}  •  {} 🥉", entity.price, format_num(&mut f, entity.qty)),
            _ => format!("{}  •  {}", entity.price, format_num(&mut f, entity.qty)),
        })
        .collect::<Vec<_>>();

    book.sort_by(|book1, book2| book2.cmp(&book1));
    book.join("\n")
}

pub fn format_message(book: ExtendedOrderBook) -> String {
    let mut f = Formatter::default();
    
    let asks = format_order_book(&mut f, book.asks);
    let bids = format_order_book(&mut f, book.bids);
    let last_price = format_num(&mut f, book.last_price);

    let msg = format!(
        "*{}*\n\nTop 10 limits of {}% depth\n\n*ASKS*\n{}\n\n*Last price* {}\n\n*BIDS*\n{}",
        book.symbol, book.depth, asks, last_price, bids
    );

    escape_markdown_v2(msg)
}
