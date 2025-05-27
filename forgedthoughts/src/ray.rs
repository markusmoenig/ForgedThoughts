use crate::{Voxel, F};
use vek::{Aabb, Vec3};

#[derive(Debug, Clone, Copy)]
pub struct Hit {
    pub position: Vec3<F>,
    pub normal: Vec3<F>,
    pub voxel: Voxel,
}

/// Ray
#[derive(PartialEq, Debug, Copy, Clone)]
pub struct Ray {
    pub origin: Vec3<F>,
    pub direction: Vec3<F>,

    pub inv_direction: Vec3<F>,

    pub sign_x: usize,
    pub sign_y: usize,
    pub sign_z: usize,
}

impl Ray {
    pub fn new(origin: Vec3<F>, direction: Vec3<F>) -> Self {
        Self {
            origin,
            direction,

            inv_direction: Vec3::new(1.0 / direction.x, 1.0 / direction.y, 1.0 / direction.z),
            sign_x: (direction.x < 0.0) as usize,
            sign_y: (direction.y < 0.0) as usize,
            sign_z: (direction.z < 0.0) as usize,
        }
    }

    pub fn at(&self, dist: &F) -> Vec3<F> {
        self.origin + *dist * self.direction
    }

    /// Intersects this ray with an AABB. Returns Some(tmin, tmax) if it hits, None otherwise.
    pub fn intersect_aabb(&self, aabb: &Aabb<F>) -> Option<(F, F)> {
        let bounds = [aabb.min, aabb.max];

        let tx_min = (bounds[self.sign_x].x - self.origin.x) * self.inv_direction.x;
        let tx_max = (bounds[1 - self.sign_x].x - self.origin.x) * self.inv_direction.x;

        let ty_min = (bounds[self.sign_y].y - self.origin.y) * self.inv_direction.y;
        let ty_max = (bounds[1 - self.sign_y].y - self.origin.y) * self.inv_direction.y;

        let tz_min = (bounds[self.sign_z].z - self.origin.z) * self.inv_direction.z;
        let tz_max = (bounds[1 - self.sign_z].z - self.origin.z) * self.inv_direction.z;

        let tmin = tx_min.max(ty_min).max(tz_min);
        let tmax = tx_max.min(ty_max).min(tz_max);

        if tmax >= tmin.max(0.0) {
            Some((tmin, tmax))
        } else {
            None
        }
    }
}
