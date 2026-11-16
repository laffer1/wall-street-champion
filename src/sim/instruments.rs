//! All tradable instruments. Every symbol is invented for this game — none
//! are real tickers.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Stock,
    Etf,
    CryptoEtf,
    Crypto,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Stock => "STOCK",
            Kind::Etf => "ETF",
            Kind::CryptoEtf => "CRYPTO ETF",
            Kind::Crypto => "CRYPTO",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Sector {
    Tech,
    Finance,
    Energy,
    Health,
    Consumer,
    Industrials,
    Staples,
    Broad,   // ETFs tracking the whole market
    Metals,  // gold trust ETF
    Crypto,  // crypto coins / crypto ETFs
}

impl Sector {
    pub fn label(self) -> &'static str {
        match self {
            Sector::Tech => "technology",
            Sector::Finance => "financials",
            Sector::Energy => "energy",
            Sector::Health => "health care",
            Sector::Consumer => "consumer",
            Sector::Industrials => "industrials",
            Sector::Staples => "consumer staples",
            Sector::Broad => "broad market",
            Sector::Metals => "precious metals",
            Sector::Crypto => "crypto",
        }
    }
}

pub struct InstrumentDef {
    pub symbol: &'static str,
    pub name: &'static str,
    pub kind: Kind,
    pub sector: Sector,
    pub start_price: f64,
    /// Base weekly volatility (log-return stdev). Real-world anchors:
    /// S&P 500 ~16% annualized => ~2.2% weekly for the index; single stocks
    /// 3-6% weekly; crypto 10-25% weekly.
    pub weekly_vol: f64,
    pub beta: f64,          // sensitivity to the market index
    pub sector_beta: f64,   // sensitivity to its sector factor
    /// Annual dividend yield (0 for non-payers).
    pub div_yield: f64,
    /// Crypto momentum (positive autocorrelation of returns).
    pub momentum: f64,
}

pub fn all_instruments() -> Vec<InstrumentDef> {
    vec![
        // ---- stocks ----
        InstrumentDef { symbol: "NBLD", name: "Nebula Dynamics", kind: Kind::Stock, sector: Sector::Tech, start_price: 84.0, weekly_vol: 0.055, beta: 1.25, sector_beta: 0.6, div_yield: 0.0, momentum: 0.0 },
        InstrumentDef { symbol: "VXP", name: "VoxelPay Technologies", kind: Kind::Stock, sector: Sector::Tech, start_price: 152.0, weekly_vol: 0.060, beta: 1.35, sector_beta: 0.7, div_yield: 0.004, momentum: 0.0 },
        InstrumentDef { symbol: "PLS", name: "Pulse Semiconductor", kind: Kind::Stock, sector: Sector::Tech, start_price: 41.5, weekly_vol: 0.065, beta: 1.45, sector_beta: 0.8, div_yield: 0.012, momentum: 0.05 },
        InstrumentDef { symbol: "MRD", name: "Meridian Retail Group", kind: Kind::Stock, sector: Sector::Consumer, start_price: 63.0, weekly_vol: 0.040, beta: 1.0, sector_beta: 0.7, div_yield: 0.028, momentum: 0.0 },
        InstrumentDef { symbol: "GRF", name: "Greenridge Foods", kind: Kind::Stock, sector: Sector::Staples, start_price: 28.0, weekly_vol: 0.028, beta: 0.5, sector_beta: 0.6, div_yield: 0.035, momentum: -0.05 },
        InstrumentDef { symbol: "FJF", name: "Fjord Fisheries", kind: Kind::Stock, sector: Sector::Staples, start_price: 19.5, weekly_vol: 0.032, beta: 0.55, sector_beta: 0.6, div_yield: 0.030, momentum: 0.0 },
        InstrumentDef { symbol: "KPT", name: "Kite Energy Corp", kind: Kind::Stock, sector: Sector::Energy, start_price: 47.0, weekly_vol: 0.048, beta: 0.9, sector_beta: 0.9, div_yield: 0.045, momentum: 0.0 },
        InstrumentDef { symbol: "HLX", name: "Helix Aerospace", kind: Kind::Stock, sector: Sector::Industrials, start_price: 118.0, weekly_vol: 0.050, beta: 1.1, sector_beta: 0.7, div_yield: 0.008, momentum: 0.0 },
        InstrumentDef { symbol: "OBF", name: "Orbital Freight Systems", kind: Kind::Stock, sector: Sector::Industrials, start_price: 73.0, weekly_vol: 0.042, beta: 1.0, sector_beta: 0.7, div_yield: 0.0, momentum: 0.0 },
        InstrumentDef { symbol: "TND", name: "Tandem Logistics", kind: Kind::Stock, sector: Sector::Industrials, start_price: 35.0, weekly_vol: 0.038, beta: 0.95, sector_beta: 0.7, div_yield: 0.018, momentum: 0.0 },
        InstrumentDef { symbol: "CYT", name: "Cypress Biotech", kind: Kind::Stock, sector: Sector::Health, start_price: 22.0, weekly_vol: 0.075, beta: 0.8, sector_beta: 0.9, div_yield: 0.0, momentum: 0.0 },
        InstrumentDef { symbol: "BRM", name: "Bramble & Root Organics", kind: Kind::Stock, sector: Sector::Consumer, start_price: 12.5, weekly_vol: 0.045, beta: 0.7, sector_beta: 0.6, div_yield: 0.0, momentum: 0.0 },
        // ---- ETFs ----
        InstrumentDef { symbol: "XGL", name: "Globex 500 Index ETF", kind: Kind::Etf, sector: Sector::Broad, start_price: 210.0, weekly_vol: 0.022, beta: 1.0, sector_beta: 0.0, div_yield: 0.014, momentum: 0.0 },
        InstrumentDef { symbol: "XEN", name: "CleanFuture Energy ETF", kind: Kind::Etf, sector: Sector::Energy, start_price: 56.0, weekly_vol: 0.038, beta: 0.85, sector_beta: 1.0, div_yield: 0.020, momentum: 0.0 },
        InstrumentDef { symbol: "XGD", name: "Aurum Gold Trust ETF", kind: Kind::Etf, sector: Sector::Metals, start_price: 95.0, weekly_vol: 0.018, beta: -0.15, sector_beta: 0.0, div_yield: 0.0, momentum: 0.0 },
        // ---- crypto ETFs ----
        InstrumentDef { symbol: "XKC", name: "Krypto Basket ETF", kind: Kind::CryptoEtf, sector: Sector::Crypto, start_price: 38.0, weekly_vol: 0.060, beta: 0.0, sector_beta: 1.2, div_yield: 0.0, momentum: 0.05 },
        InstrumentDef { symbol: "XBK", name: "ChainBlock Leaders ETF", kind: Kind::CryptoEtf, sector: Sector::Crypto, start_price: 24.0, weekly_vol: 0.070, beta: 0.0, sector_beta: 1.3, div_yield: 0.0, momentum: 0.06 },
        // ---- crypto coins ----
        InstrumentDef { symbol: "NEB", name: "Nebulium", kind: Kind::Crypto, sector: Sector::Crypto, start_price: 6.40, weekly_vol: 0.11, beta: 0.0, sector_beta: 1.1, div_yield: 0.0, momentum: 0.06 },
        InstrumentDef { symbol: "QKC", name: "QuarkCoin", kind: Kind::Crypto, sector: Sector::Crypto, start_price: 1.85, weekly_vol: 0.13, beta: 0.0, sector_beta: 1.2, div_yield: 0.0, momentum: 0.08 },
        InstrumentDef { symbol: "VLC", name: "VulcanCoin", kind: Kind::Crypto, sector: Sector::Crypto, start_price: 0.62, weekly_vol: 0.14, beta: 0.0, sector_beta: 1.3, div_yield: 0.0, momentum: 0.05 },
        InstrumentDef { symbol: "ZEP", name: "Zephyr", kind: Kind::Crypto, sector: Sector::Crypto, start_price: 12.10, weekly_vol: 0.12, beta: 0.0, sector_beta: 1.15, div_yield: 0.0, momentum: 0.07 },
        InstrumentDef { symbol: "LUM", name: "Lumen", kind: Kind::Crypto, sector: Sector::Crypto, start_price: 3.30, weekly_vol: 0.13, beta: 0.0, sector_beta: 1.25, div_yield: 0.0, momentum: 0.05 },
        InstrumentDef { symbol: "PXL", name: "PixelCoin", kind: Kind::Crypto, sector: Sector::Crypto, start_price: 0.21, weekly_vol: 0.15, beta: 0.0, sector_beta: 1.4, div_yield: 0.0, momentum: 0.09 },
    ]
}

pub fn is_earnings_week(week: u32) -> bool {
    // Quarterly reports land in weeks 6, 13, 19 and 26 of the 26-week game.
    matches!(week, 6 | 13 | 19 | 26)
}
