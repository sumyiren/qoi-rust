use core::convert::TryInto;
use std::collections::{HashMap, HashSet};

use bytemuck::cast_slice;

use crate::consts::{QOI_HEADER_SIZE, QOI_MAGIC, QOI_PIXELS_MAX};
use crate::encode_max_len;
use crate::error::{Error, Result};
use crate::types::{Channels, ColorSpace};
use crate::utils::unlikely;

/// Image Islands: dimensions, channels, color space.
pub type Point = (u32, u32);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Island {
    /// Top right of island
    pub top_left: Option<Point>,
    /// Btm left of island
    pub btm_right: Option<Point>,
}

pub struct Islands {
    pub islands: HashSet<Island>
}

impl Islands {
    /// Creates a island map
    #[inline]
    pub fn try_new(points: &HashSet<Point>, width: u32, height: u32) -> Result<Self> {

        let mut islands: HashSet<Island> = HashSet::new();
        let mut used_points: HashSet<Point> = HashSet::new();

        for point in points {
            if !used_points.contains(point) {
                let mut island = Island {

                    top_left: None,
                    btm_right: None
                };
                dfs(&points, width, height, &mut used_points, &mut island, point);
                // println!("island:{}, {}, {}, {}", island.top_left.unwrap().0, island.top_left.unwrap().1,
                //          island.btm_right.unwrap().0, island.btm_right.unwrap().1);
                islands.insert(island);
            }
        }


        Ok(Islands {
            islands
        })
    }

}

fn dfs(points: &HashSet<Point>, width: u32, height: u32, used_points: &mut HashSet<Point>, island: &mut Island, point: &Point) {
    if points.contains(&point) && !used_points.contains(&point) {

        used_points.insert(point.clone());

        if let Some(mut top_left) = island.top_left {
            if point.0 < top_left.0 {
                island.top_left = Some((point.0, top_left.1));
            }
            if point.1 < top_left.1 {
                island.top_left = Some((top_left.0, point.1));
            }
        } else {
            island.top_left = Some(point.clone());
        }

        if let Some(mut btm_right) = island.btm_right {
            if point.0 > btm_right.0 {
                island.btm_right = Some((point.0, btm_right.1));
            }
            if point.1 > btm_right.1 {
                island.btm_right = Some((btm_right.0, point.1));
            }
        } else {
            island.btm_right = Some(point.clone());
        }

        if point.0 > 0 {
            dfs(points, width, height, used_points, island,&(point.0-1, point.1));
        }

        if point.0 < width {
            dfs(points, width, height, used_points, island, &(point.0+1, point.1));
        }

        if point.1 > 0 {
            dfs(points, width, height, used_points, island, &(point.0, point.1-1));
        }

        if point.1 < height {
            dfs(points, width, height, used_points, island, &(point.0, point.1+1));
        }
    }

}

