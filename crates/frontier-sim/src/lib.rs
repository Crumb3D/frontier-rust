//! Pure Frontier: Elite II simulation — no window, no rendering.

pub mod clock;
pub mod flight;
pub mod frame;
pub mod frontier;
pub mod galaxy;
pub mod kepler;
pub mod market;
pub mod paths;
pub mod shipyard;

pub use clock::{Stardate, TimeCompression};
pub use flight::{FlightControls, FlightMode, FlightModel, ShipStats};
pub use frame::{BodyRef, FrameState, FrameSystem, WorldPos};
pub use galaxy::{build_frame_system, Galaxy, Planet, StarSystem, Starport, Station};
pub use kepler::{position_at, OrbitalElements, AU_KM};
pub use market::{Good, Market, MarketListing};
pub use shipyard::{default_eagle, fitted_to_stats, EquipDef, FittedShip, ShipDef, Shipyard};
