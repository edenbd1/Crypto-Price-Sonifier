# Crypto Price Sonifier 🎵💹

An interactive application that transforms cryptocurrency price movements into a unique audiovisual experience.

Watch and listen as prices evolve over the last 30 days.

## 🌟 Features

- **Real-time Visualization**: Interactive chart showing price evolution
- **Price Sonification**: Price variations are converted into sound
- **Visual Animations**: Dynamic bull and bear images illustrate market trends
- **Multi-crypto Support**: 
  - Bitcoin (BTC)
  - Ethereum (ETH)
  - Ripple (XRP)

## 🚀 Getting Started

### Prerequisites

- Rust (2021 edition)
- Cargo

### Installation

1. Clone the repository:

```bash
git clone https://github.com/edenbd1/Crypto-Price-Sonifier.git
```

2. Enter in the project:

```bash
cd Crypto-Price-Sonifier
```

3. Build and run the project:
```bash
cargo run
```

## 🎮 How to Use

1. Launch the application
2. Select a cryptocurrency from the main menu
3. Watch the chart draw progressively
4. Listen to price variations:
   - Higher pitch for price drops
   - Lower pitch for price increases
5. Watch bull (uptrend) and bear (downtrend) animations
6. Use the "Back to Home" button to return to the main menu

## 🛠 Tech Stack

- **GUI Framework**: egui
- **Charting**: egui_plot
- **Audio Engine**: rodio
- **Data Source**: CoinGecko API
- **Async Runtime**: tokio
- **Date Handling**: chrono
- **Image Processing**: image

## 🙏 Acknowledgments

- CoinGecko for their free API
- The Rust community for the excellent libraries

---

Built with ❤️ using Rust
