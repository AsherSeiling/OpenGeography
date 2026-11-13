//! The statistics workspace (docs/STATISTICS_WORKSPACE_PLAN.md): the result format, saved sheets of cards, and running a card.

pub mod chart;
pub mod chart_tools;
pub mod charts;
pub mod result;
pub mod sheet;
pub mod share;

pub use result::{ChartKind, ChartSpec, ColumnKind, Dropped, StatColumn, StatResult, StatTable};
pub use sheet::{CardInput, DataProfile, FieldProfile, StatCard, StatSheet, CATEGORY_PREFIX};
