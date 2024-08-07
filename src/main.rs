use hashbrown::HashSet;
use image::{ImageBuffer, RgbImage};
use rand::prelude::*;
use std::env;
use std::hash::{BuildHasher, Hash};

/* Colors works as follows:
 * 1. Pick a random color
 * 2. Find the most similar color already placed.
 * 3. Find the location of that color
 * 4. Find the nearest empty location
 * 5. Put that color in that location.
 *
 * We're going to do the dual:
 * 1. Pick a random location
 * 2. Find the closest location already placed.
 * 3. Find the color in that location.
 * 4. Find the nearest unused color
 * 5. Put that color in that location
 */

type Color = [u8; 3];
type ColorBase = [u8; 3];
fn color_base_to_color(cb: ColorBase, color_size: u8) -> Color {
    cb.map(|cbc| (cbc as u64 * 255 / (color_size - 1) as u64) as u8)
}
type ColorOffset = [i16; 3];
type Location = [usize; 2];
type LocationOffset = [isize; 2];

fn remove_random<T, H, R>(set: &mut HashSet<T, H>, rng: &mut R) -> Option<T>
where
    R: Rng,
    T: Eq + PartialEq + Hash,
    H: BuildHasher,
{
    if set.is_empty() {
        return None;
    }
    if set.capacity() >= 8 && set.len() < set.capacity() / 4 {
        set.shrink_to_fit();
    }
    let raw_table = set.raw_table_mut();
    let num_buckets = raw_table.buckets();
    loop {
        let bucket_index = rng.gen_range(0..num_buckets);
        // Safety: bucket_index is less than the number of buckets.
        // Note that we return the first time we modify the table,
        // so raw_table.buckets() never changes.
        // Also, the table has been allocated, because set is a HashSet.
        unsafe {
            if raw_table.is_bucket_full(bucket_index) {
                let bucket = raw_table.bucket(bucket_index);
                let ((element, ()), _insert_slot) = raw_table.remove(bucket);
                return Some(element);
            }
        }
    }
}

fn make_image(scale: usize, num_seeds: usize, seed: u64) -> RgbImage {
    assert!(scale > 0);
    assert!(num_seeds > 0);
    let mut rng = StdRng::seed_from_u64(seed);
    let size = scale.pow(3);
    let color_size = scale.pow(2) as u8;
    // Locations, randomly ordered.
    let mut locations: Vec<Location> = (0..size)
        .flat_map(|r| (0..size).map(move |c| [r, c]))
        .collect();
    locations.shuffle(&mut rng);
    let locations = locations;
    // Location offsets, sorted for finding most similar filled location
    let bound = (size - 1) as isize;
    let mut location_offsets: Vec<LocationOffset> = (-bound..=bound)
        .flat_map(|r| (-bound..=bound).map(move |c| [r, c]))
        .collect();
    location_offsets.sort_by_key(|[r, c]| r.pow(2) + c.pow(2));
    let location_offsets = location_offsets;
    // Map from locations to colors
    let mut grid: Vec<Vec<Option<Color>>> = vec![vec![None; size]; size];
    // Set of colors not yet used
    let mut unused_colors: HashSet<Color> = (0..color_size)
        .flat_map(|r| (0..color_size).flat_map(move |g| (0..color_size).map(move |b| [r, g, b])))
        .collect();
    let mut boundary: HashSet<Color> = HashSet::new();
    // Color offsets, sorted for finding most similar filled location
    let bound = (color_size - 1) as i16;
    let mut color_offsets: Vec<ColorOffset> = (-bound..=bound)
        .flat_map(|r| (-bound..=bound).flat_map(move |g| (-bound..=bound).map(move |b| [r, g, b])))
        .collect();
    color_offsets
        .sort_by_key(|&[r, g, b]| (r as i64).pow(2) + (g as i64).pow(2) + (b as i64).pow(2));
    let color_offsets = color_offsets;
    for (i, loc) in locations.into_iter().enumerate() {
        let printout = i % (scale.pow(6) / 100) == 0;
        if printout {
            println!("{}%", i * 100 / scale.pow(6));
        }
        let new_color = if i < num_seeds {
            remove_random(&mut unused_colors, &mut rng).expect("Don't over draw")
        } else {
            let mut count = 0;
            let old_color: ColorBase = location_offsets
                .iter()
                .filter_map(|[off0, off1]| {
                    let iloc = [loc[0] as isize + off0, loc[1] as isize + off1];
                    if iloc[0] >= 0
                        && iloc[0] < size as isize
                        && iloc[1] >= 0
                        && iloc[1] < size as isize
                    {
                        let out = grid[iloc[0] as usize][iloc[1] as usize];
                        if false && printout && out.is_none() {
                            println!("l {count}");
                            count += 1;
                        }
                        out
                    } else {
                        None
                    }
                })
                .next()
                .expect("Found");
            count = 0;
            let new_color = color_offsets
                .iter()
                .take(boundary.len())
                .filter_map(|[off_r, off_g, off_b]| {
                    let icol = [
                        old_color[0] as i16 + off_r,
                        old_color[1] as i16 + off_g,
                        old_color[2] as i16 + off_b,
                    ];
                    if icol[0] >= 0
                        && icol[0] < 256
                        && icol[1] >= 0
                        && icol[1] < 256
                        && icol[2] >= 0
                        && icol[2] < 256
                    {
                        let col = [icol[0] as u8, icol[1] as u8, icol[2] as u8];
                        if unused_colors.contains(&col) {
                            Some(col)
                        } else {
                            if false && printout {
                                println!("l {count}");
                                count += 1;
                            }
                            None
                        }
                    } else {
                        None
                    }
                })
                .next()
                .unwrap_or_else(|| {
                    *boundary
                        .iter()
                        .min_by_key(|&&[r, g, b]| {
                            (r as i64 - old_color[0] as i64).pow(2)
                                + (g as i64 - old_color[1] as i64).pow(2)
                                + (b as i64 - old_color[2] as i64).pow(2)
                        })
                        .expect("Found")
                });
            new_color
        };
        boundary.remove(&new_color);
        unused_colors.remove(&new_color);
        let [nr, ng, nb] = new_color;
        let mut neighbors = vec![];
        if nr > 0 {
            neighbors.push([nr - 1, ng, nb])
        }
        if nr < color_size - 1 {
            neighbors.push([nr + 1, ng, nb])
        }
        if ng > 0 {
            neighbors.push([nr, ng - 1, nb])
        }
        if ng < color_size - 1 {
            neighbors.push([nr, ng + 1, nb])
        }
        if nb > 0 {
            neighbors.push([nr, ng, nb - 1])
        }
        if nb < color_size - 1 {
            neighbors.push([nr, ng, nb + 1])
        }
        for ncol in neighbors {
            if unused_colors.contains(&ncol) {
                boundary.insert(ncol);
            }
        }
        grid[loc[0]][loc[1]] = Some(new_color)
    }
    let mut img: RgbImage = ImageBuffer::new(size as u32, size as u32);
    for (i, row) in grid.into_iter().enumerate() {
        for (j, color_base) in row.into_iter().enumerate() {
            if let Some(color_base) = color_base {
                img.put_pixel(
                    i as u32,
                    j as u32,
                    image::Rgb(color_base_to_color(color_base, color_size)),
                );
            }
        }
    }
    img
}
fn main() {
    let scale = env::args()
        .nth(1)
        .expect("scale given")
        .parse()
        .expect("scale num");
    let num_seeds = 2 * scale;
    let seed = 0;
    let filename = format!("img-{scale}-{num_seeds}-{seed}.png");
    println!("Start {filename}");
    let img = make_image(scale, num_seeds, seed);
    img.save(&filename).unwrap();
}
