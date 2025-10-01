# TOTAL3 coins limit orders checker
Telegram bot to check limit orders on Binance spot for TOTAL3 assets (ETH and BTC excluded).

### Main features:
- A user enters coin name, the bot validates it and checks if it's presented on Binance and tradable to USDT.
- A user can choose the order book depth to get analisys. For example, a depth of 8% means that the bot will return the largest limit orders by volume within 8% of the current price (both asks and bids).
- After receiving an order book for a symbol, bot will save it to Redis with 1 minute TTL.
- Only certian users are allowed to use the bot, their telegram ids can be changed in `configs/config.json`, so the bot is great for personal usage.
