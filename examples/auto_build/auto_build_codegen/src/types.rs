// This file was generated with `clorinde`. Do not modify.

#[cfg(feature = "chrono")]
pub mod time {
    pub type Timestamp = chrono::NaiveDateTime;
    pub type TimestampTz = chrono::DateTime<chrono::FixedOffset>;
    pub type Date = chrono::NaiveDate;
    pub type Time = chrono::NaiveTime;
}
#[cfg(feature = "time")]
pub mod time {
    pub type Timestamp = time::PrimitiveDateTime;
    pub type TimestampTz = time::OffsetDateTime;
    pub type Date = time::Date;
    pub type Time = time::Time;
}
