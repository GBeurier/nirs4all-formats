//! CF time coordinate decoding.
//!
//! Parses time units strings like "days since 1970-01-01 00:00:00" and
//! converts numeric time values to exact CF calendar date-times.
//!
//! Supported calendars:
//! - standard (mixed Gregorian/Julian)
//! - proleptic_gregorian
//! - noleap / 365_day
//! - all_leap / 366_day
//! - 360_day
//! - julian
//!
//! Reference: CF Conventions §4.4 "Time Coordinate"

use chrono::{DateTime, NaiveDate, Utc};
use std::fmt;

use crate::error::{Error, Result};
use crate::types::{NcDimension, NcGroup, NcVariable};

/// Supported CF calendar types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CfCalendar {
    /// Mixed Gregorian/Julian (default).
    Standard,
    /// Proleptic Gregorian (no Julian transition).
    ProlepticGregorian,
    /// No leap years, every year has 365 days.
    NoLeap,
    /// Every year has 366 days.
    AllLeap,
    /// Every month has 30 days (360 days/year).
    Day360,
    /// Julian calendar.
    Julian,
}

impl CfCalendar {
    /// Parse a calendar name from a CF `calendar` attribute value.
    pub fn parse(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "standard" | "gregorian" => CfCalendar::Standard,
            "proleptic_gregorian" => CfCalendar::ProlepticGregorian,
            "noleap" | "365_day" => CfCalendar::NoLeap,
            "all_leap" | "366_day" => CfCalendar::AllLeap,
            "360_day" => CfCalendar::Day360,
            "julian" => CfCalendar::Julian,
            _ => CfCalendar::Standard, // Default per CF spec
        }
    }
}

/// Time unit for CF time coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CfTimeUnit {
    Seconds,
    Minutes,
    Hours,
    Days,
    /// Calendar month offsets. Exact decoding currently accepts these only for
    /// integer offsets in the `360_day` calendar.
    Months,
}

/// Calendar date components for a CF date-time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CfDate {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

impl CfDate {
    pub fn new(year: i32, month: u8, day: u8) -> Self {
        Self { year, month, day }
    }
}

/// Time-of-day components for a CF date-time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CfTimeOfDay {
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub nanosecond: u32,
}

impl CfTimeOfDay {
    pub fn new(hour: u8, minute: u8, second: u8, nanosecond: u32) -> Result<Self> {
        validate_time(hour, minute, second, nanosecond)?;
        Ok(Self {
            hour,
            minute,
            second,
            nanosecond,
        })
    }
}

/// Exact date-time in a CF calendar.
///
/// This type can represent calendar dates that `chrono` cannot, such as
/// `360_day` February 30 or `all_leap` February 29 in a Gregorian common year.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CfDateTime {
    pub calendar: CfCalendar,
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub nanosecond: u32,
}

impl CfDateTime {
    pub fn new(calendar: CfCalendar, date: CfDate, time: CfTimeOfDay) -> Result<Self> {
        validate_date(calendar, date.year, date.month, date.day)?;
        Ok(Self {
            calendar,
            year: date.year,
            month: date.month,
            day: date.day,
            hour: time.hour,
            minute: time.minute,
            second: time.second,
            nanosecond: time.nanosecond,
        })
    }

    /// Convert to `chrono::DateTime<Utc>` using the same displayed components.
    ///
    /// This succeeds for exact CF date-times whose year/month/day exists in
    /// chrono's proleptic Gregorian calendar. Use [`decode_time_exact`] when
    /// the source calendar itself must be preserved.
    pub fn to_chrono_utc(&self) -> Result<DateTime<Utc>> {
        let date = NaiveDate::from_ymd_opt(self.year, self.month as u32, self.day as u32)
            .ok_or_else(|| {
                Error::InvalidData(format!(
                    "CF {:?} date {} cannot be represented as a Gregorian chrono date",
                    self.calendar, self
                ))
            })?;
        let datetime = date
            .and_hms_nano_opt(
                self.hour as u32,
                self.minute as u32,
                self.second as u32,
                self.nanosecond,
            )
            .ok_or_else(|| Error::InvalidData(format!("invalid CF time component in {}", self)))?;
        Ok(DateTime::<Utc>::from_naive_utc_and_offset(datetime, Utc))
    }
}

impl fmt::Display for CfDateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )?;
        if self.nanosecond != 0 {
            write!(f, ".{:09}", self.nanosecond)?;
        }
        Ok(())
    }
}

/// Parsed CF time reference.
#[derive(Debug, Clone)]
pub struct CfTimeRef {
    pub unit: CfTimeUnit,
    pub epoch: CfDateTime,
    pub calendar: CfCalendar,
}

/// A discovered CF time coordinate backed by a NetCDF coordinate variable.
#[derive(Debug, Clone)]
pub struct CfTimeCoordinate<'a> {
    /// The coordinate variable carrying CF time metadata.
    pub variable: &'a NcVariable,
    /// The dimension represented by the coordinate variable.
    pub dimension: &'a NcDimension,
    /// Parsed `units` and `calendar` metadata for decoding values.
    pub time_ref: CfTimeRef,
}

/// Parse a CF time units string like "days since 1970-01-01 00:00:00".
///
/// Format: `<unit> since <date>[ <time>]`
pub fn parse_time_units(units: &str, calendar: CfCalendar) -> Result<CfTimeRef> {
    let (unit_text, epoch_text) = split_time_units(units)?;

    let unit = match unit_text.trim().to_lowercase().as_str() {
        "second" | "seconds" | "s" => CfTimeUnit::Seconds,
        "minute" | "minutes" | "min" => CfTimeUnit::Minutes,
        "hour" | "hours" | "hr" | "h" => CfTimeUnit::Hours,
        "day" | "days" | "d" => CfTimeUnit::Days,
        "month" | "months" => CfTimeUnit::Months,
        u => {
            return Err(Error::InvalidData(format!(
                "unsupported CF time unit '{}'",
                u
            )));
        }
    };

    let epoch = parse_epoch(epoch_text.trim(), calendar)?;

    Ok(CfTimeRef {
        unit,
        epoch,
        calendar,
    })
}

/// Parse CF time metadata from a variable.
///
/// Returns `Ok(None)` when the variable has no CF time units. Invalid CF time
/// units return an error so malformed time coordinates are not silently hidden.
pub fn time_ref_from_variable(var: &NcVariable) -> Result<Option<CfTimeRef>> {
    let Some(units) = var
        .attribute("units")
        .and_then(|attr| attr.value.as_string())
    else {
        return Ok(None);
    };

    if !units.trim().to_lowercase().contains(" since ") {
        return Ok(None);
    }

    let calendar = var
        .attribute("calendar")
        .and_then(|attr| attr.value.as_string())
        .map(|value| CfCalendar::parse(&value))
        .unwrap_or(CfCalendar::Standard);

    parse_time_units(&units, calendar).map(Some)
}

/// Discover CF time coordinate variables in a group.
///
/// Only true coordinate variables are considered. Non-time coordinate variables
/// are skipped, while malformed time metadata is returned as an error.
pub fn discover_time_coordinates(group: &NcGroup) -> Result<Vec<CfTimeCoordinate<'_>>> {
    let mut coordinates = Vec::new();
    for variable in group.coordinate_variables() {
        let Some(time_ref) = time_ref_from_variable(variable)? else {
            continue;
        };
        let Some(dimension) = variable.coordinate_dimension() else {
            continue;
        };
        coordinates.push(CfTimeCoordinate {
            variable,
            dimension,
            time_ref,
        });
    }
    Ok(coordinates)
}

/// Discover the CF time coordinate used by a variable, if one exists.
pub fn discover_variable_time_coordinate<'a>(
    var: &NcVariable,
    group: &'a NcGroup,
) -> Result<Option<CfTimeCoordinate<'a>>> {
    for dimension in var.dimensions() {
        let Some(variable) = group.coordinate_variable(&dimension.name) else {
            continue;
        };
        let Some(time_ref) = time_ref_from_variable(variable)? else {
            continue;
        };
        let Some(coordinate_dimension) = variable.coordinate_dimension() else {
            continue;
        };
        return Ok(Some(CfTimeCoordinate {
            variable,
            dimension: coordinate_dimension,
            time_ref,
        }));
    }

    Ok(None)
}

fn split_time_units(units: &str) -> Result<(&str, &str)> {
    let lower = units.trim().to_lowercase();
    let Some(index) = lower.find(" since ") else {
        return Err(Error::InvalidData(format!(
            "invalid CF time units '{}': expected '<unit> since <date>'",
            units
        )));
    };
    let trimmed = units.trim();
    Ok((&trimmed[..index], &trimmed[index + " since ".len()..]))
}

/// Parse the epoch date/time string in a calendar-aware way.
fn parse_epoch(s: &str, calendar: CfCalendar) -> Result<CfDateTime> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err(Error::InvalidData("CF epoch is empty".into()));
    }

    let (date_part, time_part) = split_epoch_date_time(trimmed);
    let (year, month, day) = parse_date(date_part)?;
    let time = match time_part {
        Some(time) => parse_time_of_day(time)?,
        None => CfTimeOfDay::new(0, 0, 0, 0)?,
    };

    CfDateTime::new(calendar, CfDate::new(year, month, day), time)
}

/// Decode a numeric time value to an exact CF calendar date-time.
pub fn decode_time_exact(value: f64, time_ref: &CfTimeRef) -> Result<CfDateTime> {
    let offset = offset_from_value(value, time_ref.unit, time_ref.calendar)?;
    add_offset(time_ref.epoch, offset)
}

/// Decode numeric time values to exact CF calendar date-times.
pub fn decode_times_exact(values: &[f64], time_ref: &CfTimeRef) -> Result<Vec<CfDateTime>> {
    values
        .iter()
        .map(|&value| decode_time_exact(value, time_ref))
        .collect()
}

/// Decode a numeric time value to a UTC DateTime.
///
/// The calendar arithmetic is exact. Conversion to `chrono::DateTime<Utc>`
/// succeeds only when the exact CF date also exists in chrono's proleptic
/// Gregorian calendar, and preserves displayed components rather than the
/// source calendar. Use [`decode_time_exact`] whenever the decoded calendar
/// itself matters.
pub fn decode_time(value: f64, time_ref: &CfTimeRef) -> Result<DateTime<Utc>> {
    decode_time_exact(value, time_ref)?.to_chrono_utc()
}

/// Decode a vector of numeric time values.
pub fn decode_times(values: &[f64], time_ref: &CfTimeRef) -> Result<Vec<DateTime<Utc>>> {
    values.iter().map(|&v| decode_time(v, time_ref)).collect()
}

/// Decode numeric values using the exact CF time metadata on a variable.
pub fn decode_time_coordinate_values_exact(
    var: &NcVariable,
    values: &[f64],
) -> Result<Option<Vec<CfDateTime>>> {
    let Some(time_ref) = time_ref_from_variable(var)? else {
        return Ok(None);
    };
    decode_times_exact(values, &time_ref).map(Some)
}

/// Decode numeric values using the CF time metadata on a variable.
pub fn decode_time_coordinate_values(
    var: &NcVariable,
    values: &[f64],
) -> Result<Option<Vec<DateTime<Utc>>>> {
    let Some(time_ref) = time_ref_from_variable(var)? else {
        return Ok(None);
    };
    decode_times(values, &time_ref).map(Some)
}

const NANOS_PER_SECOND: i128 = 1_000_000_000;
const NANOS_PER_MINUTE: i128 = 60 * NANOS_PER_SECOND;
const NANOS_PER_HOUR: i128 = 60 * NANOS_PER_MINUTE;
const NANOS_PER_DAY: i128 = 24 * NANOS_PER_HOUR;

const COMMON_MONTH_LENGTHS: [u8; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
const LEAP_MONTH_LENGTHS: [u8; 12] = [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
const DAY360_MONTH_LENGTHS: [u8; 12] = [30; 12];
const STANDARD_LAST_JULIAN: (i32, u8, u8) = (1582, 10, 4);
const STANDARD_FIRST_GREGORIAN: (i32, u8, u8) = (1582, 10, 15);

#[derive(Debug, Clone, Copy)]
struct CalendarOffset {
    months: i64,
    nanoseconds: i128,
}

fn split_epoch_date_time(s: &str) -> (&str, Option<&str>) {
    if let Some((date, time)) = s.split_once('T') {
        let time = time.trim();
        return (date.trim(), (!time.is_empty()).then_some(time));
    }

    let mut parts = s.splitn(2, char::is_whitespace);
    let date = parts.next().unwrap_or("").trim();
    let time = parts.next().map(str::trim).filter(|time| !time.is_empty());
    (date, time)
}

fn parse_date(s: &str) -> Result<(i32, u8, u8)> {
    let (year_month, day) = s
        .rsplit_once('-')
        .ok_or_else(|| Error::InvalidData(format!("cannot parse CF epoch date '{}'", s)))?;
    let (year, month) = year_month
        .rsplit_once('-')
        .ok_or_else(|| Error::InvalidData(format!("cannot parse CF epoch date '{}'", s)))?;

    let year = year
        .parse::<i32>()
        .map_err(|_| Error::InvalidData(format!("cannot parse CF epoch year '{}'", year)))?;
    let month = month
        .parse::<u8>()
        .map_err(|_| Error::InvalidData(format!("cannot parse CF epoch month '{}'", month)))?;
    let day = day
        .parse::<u8>()
        .map_err(|_| Error::InvalidData(format!("cannot parse CF epoch day '{}'", day)))?;

    Ok((year, month, day))
}

fn parse_time_of_day(s: &str) -> Result<CfTimeOfDay> {
    let parts: Vec<&str> = s.split(':').collect();
    if !(2..=3).contains(&parts.len()) {
        return Err(Error::InvalidData(format!(
            "cannot parse CF epoch time '{}'",
            s
        )));
    }

    let hour = parts[0]
        .parse::<u8>()
        .map_err(|_| Error::InvalidData(format!("cannot parse CF epoch hour '{}'", parts[0])))?;
    let minute = parts[1]
        .parse::<u8>()
        .map_err(|_| Error::InvalidData(format!("cannot parse CF epoch minute '{}'", parts[1])))?;

    let (second, nanosecond) = if parts.len() == 3 {
        parse_seconds(parts[2])?
    } else {
        (0, 0)
    };

    CfTimeOfDay::new(hour, minute, second, nanosecond)
}

fn parse_seconds(s: &str) -> Result<(u8, u32)> {
    let (seconds, fraction) = match s.split_once('.') {
        Some((seconds, fraction)) => (seconds, Some(fraction)),
        None => (s, None),
    };
    let second = seconds
        .parse::<u8>()
        .map_err(|_| Error::InvalidData(format!("cannot parse CF epoch second '{}'", seconds)))?;
    let nanosecond = match fraction {
        Some("") => {
            return Err(Error::InvalidData(format!(
                "cannot parse CF epoch fractional second '{}'",
                s
            )));
        }
        Some(frac) if frac.len() <= 9 && frac.bytes().all(|byte| byte.is_ascii_digit()) => {
            let mut nanos = frac.parse::<u32>().map_err(|_| {
                Error::InvalidData(format!(
                    "cannot parse CF epoch fractional second '{}'",
                    frac
                ))
            })?;
            for _ in frac.len()..9 {
                nanos *= 10;
            }
            nanos
        }
        Some(frac) => {
            return Err(Error::InvalidData(format!(
                "CF epoch fractional second '{}' exceeds nanosecond precision",
                frac
            )));
        }
        None => 0,
    };
    Ok((second, nanosecond))
}

fn validate_date(calendar: CfCalendar, year: i32, month: u8, day: u8) -> Result<()> {
    if !(1..=12).contains(&month) {
        return Err(Error::InvalidData(format!(
            "invalid {:?} month {}",
            calendar, month
        )));
    }
    let max_day = month_length(calendar, year as i128, month);
    if day == 0 || day > max_day {
        return Err(Error::InvalidData(format!(
            "invalid {:?} date {:04}-{:02}-{:02}",
            calendar, year, month, day
        )));
    }
    if calendar == CfCalendar::Standard
        && compare_ymd((year, month, day), STANDARD_LAST_JULIAN).is_gt()
        && compare_ymd((year, month, day), STANDARD_FIRST_GREGORIAN).is_lt()
    {
        return Err(Error::InvalidData(format!(
            "invalid standard calendar date in Gregorian reform gap {:04}-{:02}-{:02}",
            year, month, day
        )));
    }
    Ok(())
}

fn validate_time(hour: u8, minute: u8, second: u8, nanosecond: u32) -> Result<()> {
    if hour > 23 || minute > 59 || second > 59 || nanosecond >= 1_000_000_000 {
        return Err(Error::InvalidData(format!(
            "invalid CF time {:02}:{:02}:{:02}.{:09}",
            hour, minute, second, nanosecond
        )));
    }
    Ok(())
}

fn offset_from_value(value: f64, unit: CfTimeUnit, calendar: CfCalendar) -> Result<CalendarOffset> {
    if !value.is_finite() {
        return Err(Error::InvalidData(format!(
            "CF time value {} is not finite",
            value
        )));
    }

    match unit {
        CfTimeUnit::Seconds => Ok(CalendarOffset {
            months: 0,
            nanoseconds: rounded_i128(value * NANOS_PER_SECOND as f64, "CF seconds offset")?,
        }),
        CfTimeUnit::Minutes => Ok(CalendarOffset {
            months: 0,
            nanoseconds: rounded_i128(value * NANOS_PER_MINUTE as f64, "CF minutes offset")?,
        }),
        CfTimeUnit::Hours => Ok(CalendarOffset {
            months: 0,
            nanoseconds: rounded_i128(value * NANOS_PER_HOUR as f64, "CF hours offset")?,
        }),
        CfTimeUnit::Days => Ok(CalendarOffset {
            months: 0,
            nanoseconds: rounded_i128(value * NANOS_PER_DAY as f64, "CF days offset")?,
        }),
        CfTimeUnit::Months => {
            let months = integer_i64(value, "CF month offset")?;
            if calendar != CfCalendar::Day360 {
                return Err(Error::InvalidData(format!(
                    "CF month offsets are exact only for the 360_day calendar, got {:?}",
                    calendar
                )));
            }
            Ok(CalendarOffset {
                months,
                nanoseconds: 0,
            })
        }
    }
}

fn rounded_i128(value: f64, context: &str) -> Result<i128> {
    if !value.is_finite() || value < i128::MIN as f64 || value > i128::MAX as f64 {
        return Err(Error::InvalidData(format!("{context} is out of range")));
    }
    Ok(value.round() as i128)
}

fn integer_i64(value: f64, context: &str) -> Result<i64> {
    if value.fract() != 0.0 {
        return Err(Error::InvalidData(format!("{context} must be an integer")));
    }
    let integer = rounded_i128(value, context)?;
    i64::try_from(integer).map_err(|_| Error::InvalidData(format!("{context} is out of range")))
}

fn add_offset(epoch: CfDateTime, offset: CalendarOffset) -> Result<CfDateTime> {
    let epoch = if offset.months == 0 {
        epoch
    } else {
        add_months(epoch, offset.months)?
    };

    add_nanoseconds(epoch, offset.nanoseconds)
}

fn add_months(epoch: CfDateTime, months: i64) -> Result<CfDateTime> {
    let month_index = epoch.year as i128 * 12 + i128::from(epoch.month - 1) + i128::from(months);
    let year = floor_div(month_index, 12);
    let month = (month_index - year * 12 + 1) as u8;
    let year = checked_i32(year, "CF month offset year")?;

    CfDateTime::new(
        epoch.calendar,
        CfDate::new(year, month, epoch.day),
        CfTimeOfDay::new(epoch.hour, epoch.minute, epoch.second, epoch.nanosecond)?,
    )
}

fn add_nanoseconds(epoch: CfDateTime, nanoseconds: i128) -> Result<CfDateTime> {
    let time_nanos = i128::from(epoch.hour) * NANOS_PER_HOUR
        + i128::from(epoch.minute) * NANOS_PER_MINUTE
        + i128::from(epoch.second) * NANOS_PER_SECOND
        + i128::from(epoch.nanosecond);
    let total_nanos = time_nanos
        .checked_add(nanoseconds)
        .ok_or_else(|| Error::InvalidData("CF time offset exceeds i128 capacity".into()))?;
    let day_delta = floor_div(total_nanos, NANOS_PER_DAY);
    let nanos_of_day = total_nanos - day_delta * NANOS_PER_DAY;

    let day_number = day_number_from_date(epoch.calendar, epoch.year, epoch.month, epoch.day)?
        .checked_add(day_delta)
        .ok_or_else(|| Error::InvalidData("CF date offset exceeds i128 capacity".into()))?;
    let (year, month, day) = date_from_day_number(epoch.calendar, day_number)?;
    let (hour, minute, second, nanosecond) = split_nanos_of_day(nanos_of_day);

    CfDateTime::new(
        epoch.calendar,
        CfDate::new(year, month, day),
        CfTimeOfDay::new(hour, minute, second, nanosecond)?,
    )
}

fn split_nanos_of_day(nanos: i128) -> (u8, u8, u8, u32) {
    let hour = nanos / NANOS_PER_HOUR;
    let nanos = nanos - hour * NANOS_PER_HOUR;
    let minute = nanos / NANOS_PER_MINUTE;
    let nanos = nanos - minute * NANOS_PER_MINUTE;
    let second = nanos / NANOS_PER_SECOND;
    let nanosecond = nanos - second * NANOS_PER_SECOND;
    (hour as u8, minute as u8, second as u8, nanosecond as u32)
}

fn day_number_from_date(calendar: CfCalendar, year: i32, month: u8, day: u8) -> Result<i128> {
    validate_date(calendar, year, month, day)?;
    if calendar == CfCalendar::Standard {
        return Ok(standard_day_number(year, month, day));
    }
    Ok(days_before_year(calendar, year as i128)
        + days_before_month(calendar, year as i128, month)
        + i128::from(day - 1))
}

fn date_from_day_number(calendar: CfCalendar, day_number: i128) -> Result<(i32, u8, u8)> {
    match calendar {
        CfCalendar::NoLeap => fixed_year_date(calendar, day_number, 365),
        CfCalendar::AllLeap => fixed_year_date(calendar, day_number, 366),
        CfCalendar::Day360 => day360_date(day_number),
        CfCalendar::Julian => julian_date(day_number),
        CfCalendar::Standard => standard_date(day_number),
        CfCalendar::ProlepticGregorian => gregorian_date(day_number),
    }
}

fn fixed_year_date(
    calendar: CfCalendar,
    day_number: i128,
    days_per_year: i128,
) -> Result<(i32, u8, u8)> {
    let year = floor_div(day_number, days_per_year);
    let day_of_year = day_number - year * days_per_year;
    date_from_year_day(calendar, year, day_of_year)
}

fn day360_date(day_number: i128) -> Result<(i32, u8, u8)> {
    let year = floor_div(day_number, 360);
    let day_of_year = day_number - year * 360;
    let month = day_of_year / 30 + 1;
    let day = day_of_year % 30 + 1;
    Ok((
        checked_i32(year, "CF 360_day year")?,
        month as u8,
        day as u8,
    ))
}

fn julian_date(day_number: i128) -> Result<(i32, u8, u8)> {
    let cycle = floor_div(day_number, 1_461);
    let mut day_in_cycle = day_number - cycle * 1_461;
    let mut year = cycle * 4;

    let day_of_year = if day_in_cycle < 366 {
        day_in_cycle
    } else {
        day_in_cycle -= 366;
        year += 1 + day_in_cycle / 365;
        day_in_cycle % 365
    };

    date_from_year_day(CfCalendar::Julian, year, day_of_year)
}

fn gregorian_date(day_number: i128) -> Result<(i32, u8, u8)> {
    let cycle = floor_div(day_number, 146_097);
    let day_in_cycle = day_number - cycle * 146_097;
    let mut lo = 0i128;
    let mut hi = 400i128;
    while lo + 1 < hi {
        let mid = (lo + hi) / 2;
        if days_before_gregorian_year(mid) <= day_in_cycle {
            lo = mid;
        } else {
            hi = mid;
        }
    }

    let year = cycle * 400 + lo;
    let day_of_year = day_in_cycle - days_before_gregorian_year(lo);
    date_from_year_day(CfCalendar::ProlepticGregorian, year, day_of_year)
}

fn standard_date(day_number: i128) -> Result<(i32, u8, u8)> {
    let reform_start = julian_day_number_raw(
        STANDARD_LAST_JULIAN.0,
        STANDARD_LAST_JULIAN.1,
        STANDARD_LAST_JULIAN.2,
    ) + 1;

    if day_number < reform_start {
        julian_date(day_number)
    } else {
        gregorian_date(day_number - standard_gregorian_offset())
    }
}

fn date_from_year_day(
    calendar: CfCalendar,
    year: i128,
    mut day_of_year: i128,
) -> Result<(i32, u8, u8)> {
    let year_i32 = checked_i32(year, "CF decoded year")?;
    for month in 1..=12 {
        let month_len = i128::from(month_length(calendar, year, month));
        if day_of_year < month_len {
            return Ok((year_i32, month, (day_of_year + 1) as u8));
        }
        day_of_year -= month_len;
    }

    Err(Error::InvalidData(format!(
        "CF day-of-year {} is invalid for {:?} year {}",
        day_of_year, calendar, year
    )))
}

fn days_before_year(calendar: CfCalendar, year: i128) -> i128 {
    match calendar {
        CfCalendar::NoLeap => year * 365,
        CfCalendar::AllLeap => year * 366,
        CfCalendar::Day360 => year * 360,
        CfCalendar::Julian => year * 365 + floor_div(year + 3, 4),
        CfCalendar::Standard | CfCalendar::ProlepticGregorian => days_before_gregorian_year(year),
    }
}

fn days_before_gregorian_year(year: i128) -> i128 {
    year * 365 + floor_div(year + 3, 4) - floor_div(year + 99, 100) + floor_div(year + 399, 400)
}

fn days_before_month(calendar: CfCalendar, year: i128, month: u8) -> i128 {
    (1..month)
        .map(|candidate| i128::from(month_length(calendar, year, candidate)))
        .sum()
}

fn standard_day_number(year: i32, month: u8, day: u8) -> i128 {
    if compare_ymd((year, month, day), STANDARD_LAST_JULIAN).is_le() {
        julian_day_number_raw(year, month, day)
    } else {
        gregorian_day_number_raw(year, month, day) + standard_gregorian_offset()
    }
}

fn standard_gregorian_offset() -> i128 {
    julian_day_number_raw(
        STANDARD_LAST_JULIAN.0,
        STANDARD_LAST_JULIAN.1,
        STANDARD_LAST_JULIAN.2,
    ) + 1
        - gregorian_day_number_raw(
            STANDARD_FIRST_GREGORIAN.0,
            STANDARD_FIRST_GREGORIAN.1,
            STANDARD_FIRST_GREGORIAN.2,
        )
}

fn julian_day_number_raw(year: i32, month: u8, day: u8) -> i128 {
    days_before_year(CfCalendar::Julian, year as i128)
        + days_before_month(CfCalendar::Julian, year as i128, month)
        + i128::from(day - 1)
}

fn gregorian_day_number_raw(year: i32, month: u8, day: u8) -> i128 {
    days_before_year(CfCalendar::ProlepticGregorian, year as i128)
        + days_before_month(CfCalendar::ProlepticGregorian, year as i128, month)
        + i128::from(day - 1)
}

fn month_length(calendar: CfCalendar, year: i128, month: u8) -> u8 {
    let index = usize::from(month - 1);
    match calendar {
        CfCalendar::NoLeap => COMMON_MONTH_LENGTHS[index],
        CfCalendar::AllLeap => LEAP_MONTH_LENGTHS[index],
        CfCalendar::Day360 => DAY360_MONTH_LENGTHS[index],
        CfCalendar::Julian => {
            if is_julian_leap_year(year) {
                LEAP_MONTH_LENGTHS[index]
            } else {
                COMMON_MONTH_LENGTHS[index]
            }
        }
        CfCalendar::Standard | CfCalendar::ProlepticGregorian => {
            if is_gregorian_leap_year(year) {
                LEAP_MONTH_LENGTHS[index]
            } else {
                COMMON_MONTH_LENGTHS[index]
            }
        }
    }
}

fn is_julian_leap_year(year: i128) -> bool {
    year.rem_euclid(4) == 0
}

fn is_gregorian_leap_year(year: i128) -> bool {
    year.rem_euclid(4) == 0 && (year.rem_euclid(100) != 0 || year.rem_euclid(400) == 0)
}

fn checked_i32(value: i128, context: &str) -> Result<i32> {
    i32::try_from(value).map_err(|_| Error::InvalidData(format!("{context} is out of range")))
}

fn compare_ymd(left: (i32, u8, u8), right: (i32, u8, u8)) -> std::cmp::Ordering {
    left.0
        .cmp(&right.0)
        .then_with(|| left.1.cmp(&right.1))
        .then_with(|| left.2.cmp(&right.2))
}

fn floor_div(numerator: i128, denominator: i128) -> i128 {
    debug_assert!(denominator > 0);
    let quotient = numerator / denominator;
    let remainder = numerator % denominator;
    if remainder < 0 {
        quotient - 1
    } else {
        quotient
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{NcAttrValue, NcAttribute, NcType};

    fn attr(name: &str, value: &str) -> NcAttribute {
        NcAttribute {
            name: name.into(),
            value: NcAttrValue::Chars(value.into()),
        }
    }

    fn coordinate_var(name: &str, size: u64, attrs: Vec<NcAttribute>) -> NcVariable {
        NcVariable {
            name: name.into(),
            dimensions: vec![NcDimension {
                name: name.into(),
                size,
                is_unlimited: false,
            }],
            dtype: NcType::Double,
            attributes: attrs,
            data_offset: 0,
            _data_size: 0,
            is_record_var: false,
            record_size: 0,
        }
    }

    fn dt(calendar: CfCalendar, date: (i32, u8, u8), time: (u8, u8, u8, u32)) -> CfDateTime {
        CfDateTime::new(
            calendar,
            CfDate::new(date.0, date.1, date.2),
            CfTimeOfDay::new(time.0, time.1, time.2, time.3).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn parse_days_since() {
        let tr = parse_time_units("days since 1970-01-01", CfCalendar::Standard).unwrap();
        assert_eq!(tr.unit, CfTimeUnit::Days);
        assert_eq!(
            tr.epoch,
            dt(CfCalendar::Standard, (1970, 1, 1), (0, 0, 0, 0))
        );
    }

    #[test]
    fn parse_hours_since_with_time() {
        let tr = parse_time_units("hours since 2000-01-01 00:00:00", CfCalendar::Standard).unwrap();
        assert_eq!(tr.unit, CfTimeUnit::Hours);
        assert_eq!(
            tr.epoch,
            dt(CfCalendar::Standard, (2000, 1, 1), (0, 0, 0, 0))
        );
    }

    #[test]
    fn decode_days() {
        let tr = parse_time_units("days since 1970-01-01", CfCalendar::Standard).unwrap();
        let dt = decode_time(365.0, &tr).unwrap();
        assert_eq!(dt.format("%Y-%m-%d").to_string(), "1971-01-01");
    }

    #[test]
    fn decode_hours() {
        let tr = parse_time_units("hours since 2000-01-01 00:00:00", CfCalendar::Standard).unwrap();
        let dt = decode_time(24.0, &tr).unwrap();
        assert_eq!(dt.format("%Y-%m-%d").to_string(), "2000-01-02");
    }

    #[test]
    fn calendar_from_str() {
        assert_eq!(CfCalendar::parse("standard"), CfCalendar::Standard);
        assert_eq!(CfCalendar::parse("noleap"), CfCalendar::NoLeap);
        assert_eq!(CfCalendar::parse("365_day"), CfCalendar::NoLeap);
        assert_eq!(CfCalendar::parse("360_day"), CfCalendar::Day360);
        assert_eq!(
            CfCalendar::parse("proleptic_gregorian"),
            CfCalendar::ProlepticGregorian
        );
    }

    #[test]
    fn invalid_units() {
        assert!(parse_time_units("invalid", CfCalendar::Standard).is_err());
        assert!(parse_time_units("furlongs since yesterday", CfCalendar::Standard).is_err());
    }

    #[test]
    fn standard_calendar_uses_gregorian_reform_transition() {
        let tr = parse_time_units("days since 1582-10-04", CfCalendar::Standard).unwrap();

        let exact = decode_time_exact(1.0, &tr).unwrap();
        assert_eq!(
            exact,
            dt(CfCalendar::Standard, (1582, 10, 15), (0, 0, 0, 0))
        );
        assert!(parse_time_units("days since 1582-10-10", CfCalendar::Standard).is_err());
    }

    #[test]
    fn parse_360_day_epoch_with_february_30() {
        let tr =
            parse_time_units("days since 2000-02-30 06:30:15.250", CfCalendar::Day360).unwrap();

        assert_eq!(
            tr.epoch,
            dt(CfCalendar::Day360, (2000, 2, 30), (6, 30, 15, 250_000_000))
        );
        assert!(parse_time_units("days since 2000-02-31", CfCalendar::Day360).is_err());
    }

    #[test]
    fn noleap_calendar_skips_february_29() {
        let tr = parse_time_units("days since 2000-01-01", CfCalendar::NoLeap).unwrap();

        let exact = decode_time_exact(59.0, &tr).unwrap();
        assert_eq!(exact, dt(CfCalendar::NoLeap, (2000, 3, 1), (0, 0, 0, 0)));

        let chrono = decode_time(59.0, &tr).unwrap();
        assert_eq!(chrono.format("%Y-%m-%d").to_string(), "2000-03-01");
    }

    #[test]
    fn all_leap_calendar_has_february_29_every_year() {
        let tr = parse_time_units("days since 2001-02-28", CfCalendar::AllLeap).unwrap();

        let exact = decode_time_exact(1.0, &tr).unwrap();
        assert_eq!(exact, dt(CfCalendar::AllLeap, (2001, 2, 29), (0, 0, 0, 0)));
        assert!(decode_time(1.0, &tr).is_err());
    }

    #[test]
    fn calendar_360_day_uses_uniform_30_day_months() {
        let tr = parse_time_units("hours since 2000-01-30 12:00:00", CfCalendar::Day360).unwrap();

        let exact = decode_time_exact(12.0, &tr).unwrap();
        assert_eq!(exact, dt(CfCalendar::Day360, (2000, 2, 1), (0, 0, 0, 0)));

        let feb30 = parse_time_units("days since 2000-01-01", CfCalendar::Day360).unwrap();
        let exact = decode_time_exact(59.0, &feb30).unwrap();
        assert_eq!(exact, dt(CfCalendar::Day360, (2000, 2, 30), (0, 0, 0, 0)));
        assert!(decode_time(59.0, &feb30).is_err());
    }

    #[test]
    fn julian_calendar_leap_years_are_exact() {
        let tr = parse_time_units("days since 1900-02-28", CfCalendar::Julian).unwrap();

        let exact = decode_time_exact(1.0, &tr).unwrap();
        assert_eq!(exact, dt(CfCalendar::Julian, (1900, 2, 29), (0, 0, 0, 0)));
        assert!(decode_time(1.0, &tr).is_err());

        let exact = decode_time_exact(2.0, &tr).unwrap();
        assert_eq!(exact, dt(CfCalendar::Julian, (1900, 3, 1), (0, 0, 0, 0)));
    }

    #[test]
    fn negative_offsets_use_calendar_arithmetic() {
        let tr = parse_time_units("days since 2001-01-01", CfCalendar::NoLeap).unwrap();

        let exact = decode_time_exact(-1.0, &tr).unwrap();
        assert_eq!(exact, dt(CfCalendar::NoLeap, (2000, 12, 31), (0, 0, 0, 0)));
    }

    #[test]
    fn calendar_360_day_integer_month_offsets() {
        let tr = parse_time_units("months since 2000-01-30", CfCalendar::Day360).unwrap();

        let exact = decode_time_exact(1.0, &tr).unwrap();
        assert_eq!(exact, dt(CfCalendar::Day360, (2000, 2, 30), (0, 0, 0, 0)));

        assert!(decode_time_exact(0.5, &tr).is_err());
        let gregorian_months =
            parse_time_units("months since 2000-01-30", CfCalendar::ProlepticGregorian).unwrap();
        assert!(decode_time_exact(1.0, &gregorian_months).is_err());
    }

    #[test]
    fn time_reference_from_variable_uses_units_and_calendar() {
        let var = coordinate_var(
            "time",
            3,
            vec![
                attr("units", "hours since 2001-01-01 12:00:00"),
                attr("calendar", "proleptic_gregorian"),
            ],
        );

        let time_ref = time_ref_from_variable(&var).unwrap().unwrap();
        assert_eq!(time_ref.unit, CfTimeUnit::Hours);
        assert_eq!(time_ref.calendar, CfCalendar::ProlepticGregorian);
        assert_eq!(
            time_ref.epoch,
            dt(CfCalendar::ProlepticGregorian, (2001, 1, 1), (12, 0, 0, 0))
        );
    }

    #[test]
    fn discover_time_coordinates_only_uses_coordinate_variables() {
        let time = coordinate_var("time", 3, vec![attr("units", "days since 1970-01-01")]);
        let data_time = NcVariable {
            name: "data_time".into(),
            dimensions: vec![NcDimension {
                name: "obs".into(),
                size: 3,
                is_unlimited: false,
            }],
            dtype: NcType::Double,
            attributes: vec![attr("units", "days since 1970-01-01")],
            data_offset: 0,
            _data_size: 0,
            is_record_var: false,
            record_size: 0,
        };
        let group = NcGroup {
            name: "/".into(),
            dimensions: vec![time.dimensions()[0].clone()],
            variables: vec![data_time, time],
            attributes: vec![],
            groups: vec![],
        };

        let times = discover_time_coordinates(&group).unwrap();
        assert_eq!(times.len(), 1);
        assert_eq!(times[0].variable.name(), "time");
        assert_eq!(times[0].dimension.name, "time");
    }

    #[test]
    fn discovers_variable_time_coordinate() {
        let time = coordinate_var("time", 3, vec![attr("units", "hours since 2000-01-01")]);
        let lat = coordinate_var("lat", 2, vec![attr("units", "degrees_north")]);
        let temperature = NcVariable {
            name: "temperature".into(),
            dimensions: vec![time.dimensions()[0].clone(), lat.dimensions()[0].clone()],
            dtype: NcType::Float,
            attributes: vec![],
            data_offset: 0,
            _data_size: 0,
            is_record_var: false,
            record_size: 0,
        };
        let group = NcGroup {
            name: "/".into(),
            dimensions: vec![time.dimensions()[0].clone(), lat.dimensions()[0].clone()],
            variables: vec![lat, time, temperature.clone()],
            attributes: vec![],
            groups: vec![],
        };

        let discovered = discover_variable_time_coordinate(&temperature, &group)
            .unwrap()
            .unwrap();
        assert_eq!(discovered.variable.name(), "time");
        assert_eq!(discovered.time_ref.unit, CfTimeUnit::Hours);
    }

    #[test]
    fn time_coordinate_values_decode_from_units() {
        let var = coordinate_var("time", 2, vec![attr("units", "days since 1970-01-01")]);

        let decoded = decode_time_coordinate_values(&var, &[0.0, 1.0])
            .unwrap()
            .unwrap();
        assert_eq!(decoded[0].format("%Y-%m-%d").to_string(), "1970-01-01");
        assert_eq!(decoded[1].format("%Y-%m-%d").to_string(), "1970-01-02");
    }
}
