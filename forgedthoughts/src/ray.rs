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
    pub dir: Vec3<F>,

    pub inv_direction: Vec3<F>,

    pub sign_x: usize,
    pub sign_y: usize,
    pub sign_z: usize,
}

impl Ray {
    pub fn new(origin: Vec3<F>, dir: Vec3<F>) -> Self {
        Self {
            origin,
            dir,

            inv_direction: Vec3::new(1.0 / dir.x, 1.0 / dir.y, 1.0 / dir.z),
            sign_x: (dir.x < 0.0) as usize,
            sign_y: (dir.y < 0.0) as usize,
            sign_z: (dir.z < 0.0) as usize,
        }
    }

    /// Returns the position by `dist` units along its direction.
    pub fn at(&self, dist: &F) -> Vec3<F> {
        self.origin + *dist * self.dir
    }

    /// Returns a new Ray advanced by `dist` units along its direction.
    #[inline(always)]
    pub fn advanced(&self, dist: F) -> Self {
        Self::new(self.at(&dist), self.dir)
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

    /// Intersects the ray with a sphere.
    /// Returns `Some(t)` where `t` is the distance along the ray to the intersection,
    /// or `None` if there is no hit.
    pub fn intersect_sphere(&self, center: Vec3<F>, radius: F) -> Option<F> {
        let oc = center - self.origin;
        let b = oc.dot(self.dir);
        let det = b * b - oc.dot(oc) + radius * radius;

        if det < 0.0 {
            None
        } else {
            let sqrt_det = det.sqrt();
            let t1 = b - sqrt_det;
            let t2 = b + sqrt_det;
            let epsilon = 1e-3;

            if t1 > epsilon {
                Some(t1)
            } else if t2 > epsilon {
                Some(t2)
            } else {
                None
            }
        }
    }
}
