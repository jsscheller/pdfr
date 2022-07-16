use anyhow::{anyhow, Error, Result};
use serde::Deserialize;
use std::ops::RangeInclusive;
use std::str::FromStr;

#[derive(Clone)]
pub struct Intervals {
    items: Vec<Interval>,
}

#[derive(Clone)]
struct Interval {
    start: usize,
    end: Option<usize>,
}

impl Intervals {
    pub fn iter(&self, max: usize) -> IntervalsIterator {
        IntervalsIterator {
            ivs: self,
            offset: 0,
            range: None,
            max,
        }
    }
}

impl From<RangeInclusive<usize>> for Intervals {
    fn from(range: RangeInclusive<usize>) -> Self {
        Self {
            items: vec![Interval {
                start: *range.start(),
                end: Some(*range.end()),
            }],
        }
    }
}

impl FromStr for Intervals {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let mut items = vec![];
        for item in s.split(',') {
            let dash = item.find('-');
            let mut iv = Interval {
                start: item[..dash.unwrap_or(item.len())].parse()?,
                end: None,
            };
            if let Some(dash) = dash {
                if item.len() != dash + 1 {
                    iv.end = Some(item[dash + 1..].parse()?);
                }
            } else {
                iv.end = Some(iv.start);
            }
            if iv.end.is_some() && iv.end.unwrap() < iv.start {
                return Err(anyhow!(
                    "invalid interval - end value must be greater than the start value"
                ));
            }
            items.push(iv);
        }

        Ok(Self { items })
    }
}

pub struct IntervalsIterator<'a> {
    ivs: &'a Intervals,
    offset: usize,
    range: Option<RangeInclusive<usize>>,
    max: usize,
}

impl<'a> Iterator for IntervalsIterator<'a> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(range) = self.range.as_mut() {
            if let Some(item) = range.next() {
                Some(item)
            } else {
                self.range = None;
                self.next()
            }
        } else if let Some(iv) = self.ivs.items.get(self.offset) {
            self.offset += 1;
            self.range = Some(RangeInclusive::new(iv.start, iv.end.unwrap_or(self.max)));
            self.next()
        } else {
            None
        }
    }
}

pub struct Size {
    pub width: Option<f32>,
    pub height: Option<f32>,
}

impl FromStr for Size {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let mut size = Self {
            width: None,
            height: None,
        };
        let s = s.to_lowercase();
        if s.contains('x') {
            let mut split = s.split('x').map(|n| n.parse::<f32>());
            size.width = split.next().map_or(None, |n| n.ok());
            size.height = split.next().map_or(None, |n| n.ok());
        } else {
            size.width = Some(s.parse::<f32>().unwrap());
        }

        Ok(size)
    }
}

#[derive(Default, Debug, Clone, Copy, Deserialize)]
pub struct Geometry {
    pub width: f64,
    pub height: f64,
    pub x: f64,
    pub y: f64,
}

impl FromStr for Geometry {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        return parse_geom(s).ok_or_else(|| anyhow!("invalid geometry"));

        fn parse_geom(s: &str) -> Option<Geometry> {
            let mut offsets = s
                .chars()
                .enumerate()
                .filter(|(_, c)| !c.is_numeric())
                .map(|(pos, _)| pos);
            let size = offsets.next()?;
            let x = offsets.next()?;
            let y = offsets.next()?;
            Some(Geometry {
                width: s[..size].parse().ok()?,
                height: s[size + 1..x].parse().ok()?,
                x: s[x + 1..y].parse().ok()?,
                y: s.get(y + 1..)?.parse().ok()?,
            })
        }
    }
}

#[derive(Default, Debug, Clone, Copy, Deserialize)]
pub struct Coords {
    pub x: f64,
    pub y: f64,
}

impl FromStr for Coords {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        return parse_coords(s).ok_or_else(|| anyhow!("invalid coordinates"));

        fn parse_coords(s: &str) -> Option<Coords> {
            let mut offsets = s
                .chars()
                .enumerate()
                .filter(|(_, c)| !c.is_numeric())
                .map(|(pos, _)| pos);
            let x = offsets.next()?;
            let y = offsets.next()?;
            Some(Coords {
                x: s[x + 1..y].parse().ok()?,
                y: s.get(y + 1..)?.parse().ok()?,
            })
        }
    }
}
