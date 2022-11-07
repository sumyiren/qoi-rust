use std::collections::{HashSet, VecDeque};
use std::mem::transmute;
use bytemuck::cast_slice;
use crate::error::{Result};
use crate::utils::{Writer};
use rayon::prelude::*;




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
    pub(crate) fn find_islands(points: &HashSet<Point>) -> Vec<Island> {

        let mut islands: Vec<Island> = Vec::new();
        let mut used_points: HashSet<Point> = HashSet::new();

        for point in points {
            if !used_points.contains(point) {
                let mut island = Island {
                    top_left: None,
                    btm_right: None
                };
                bfs(&points, &mut used_points, &mut island, point);
                // println!("island:{}, {}, {}, {}", island.top_left.unwrap().0, island.top_left.unwrap().1,
                //          island.btm_right.unwrap().0, island.btm_right.unwrap().1);
                if island.top_left != None && island.btm_right != None {
                    islands.push(island);
                }
                // println!("points - used_points:{}, {}", points.len(), used_points.len());
            }
        }

        islands
    }

    /// Serializes the header into a bytes array.
    #[inline]
    pub(crate) fn encode<W: Writer>(mut buf: W, islands: &Vec<Island>) -> Result<W> {

        for island in islands {
            let top_left = island.top_left.unwrap();
            let btm_right = island.btm_right.unwrap();

            let bytes: [u8; 4] = unsafe { transmute(top_left.0.to_be()) };
            buf = buf.write_many(&bytes).unwrap();
            let bytes: [u8; 4] = unsafe { transmute(top_left.1.to_be()) };
            buf = buf.write_many(&bytes).unwrap();
            let bytes: [u8; 4] = unsafe { transmute(btm_right.0.to_be()) };
            buf = buf.write_many(&bytes).unwrap();
            let bytes: [u8; 4] = unsafe { transmute(btm_right.1.to_be()) };
            buf = buf.write_many(&bytes).unwrap();
        }

        Ok(buf)
    }

    /// Deserializes the header from a byte array.
    #[inline]
    pub(crate) fn decode(data: impl AsRef<[u8]>, n_islands: u32) -> Result<Self> {

        let chunk_size = 16;
        let data = &data.as_ref();

        let mut islands: HashSet<Island> = Default::default();
        let chunks_iter = data.chunks(chunk_size);

        let mut islands_count = 0;
        for chunk in chunks_iter {
            islands_count += 1;
            if islands_count > n_islands {
                break
            }
            let v = cast_slice::<_, [u8; 4]>(&chunk);
            let top_left = (u32::from_be_bytes(v[0]), u32::from_be_bytes(v[1]));
            let btm_right = (u32::from_be_bytes(v[2]), u32::from_be_bytes(v[3]));
            let island = Island {
                top_left: Some(top_left),
                btm_right: Some(btm_right)
            };
            islands.insert(island);
        }

        // Self::try_new(width, height, channels, colorspace)
        Ok(Islands { islands })
    }
}

fn bfs(points: &HashSet<Point>, used_points: &mut HashSet<Point>, island: &mut Island, point: &Point) {
    let mut q = VecDeque::new();
    q.push_back(point.clone());

    while let Some(point) = q.pop_front() {
        if points.contains(&point) && !used_points.contains(&point) {
            used_points.insert(point.clone());

            if let Some(top_left) = island.top_left {
                if point.0 < top_left.0 {
                    island.top_left = Some((point.0, top_left.1));
                }
                if point.1 < top_left.1 {
                    island.top_left = Some((top_left.0, point.1));
                }
            } else {
                island.top_left = Some(point.clone());
            }

            if let Some(btm_right) = island.btm_right {
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
                q.push_back((point.0 - 1, point.1));
            }

            // if point.0 < width {
                q.push_back((point.0 + 1, point.1));
            // }

            if point.1 > 0 {
                q.push_back((point.0, point.1 - 1));
            }

            // if point.1 < height {
                q.push_back((point.0, point.1 + 1));
            // }
        }
    }
}

