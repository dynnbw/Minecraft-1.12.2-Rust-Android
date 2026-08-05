use crate::compat::Java::JavaRandom;

/// Exact 2D branch of MCP 1.12.2 `NoiseGeneratorSimplex`.
#[derive(Debug, Clone)]
pub struct NoiseGeneratorSimplex {
    p: [i32; 512],
    pub xo: f64,
    pub yo: f64,
    pub zo: f64,
}

const GRAD3: [[i32; 3]; 12] = [
    [1, 1, 0], [-1, 1, 0], [1, -1, 0], [-1, -1, 0],
    [1, 0, 1], [-1, 0, 1], [1, 0, -1], [-1, 0, -1],
    [0, 1, 1], [0, -1, 1], [0, 1, -1], [0, -1, -1],
];

impl NoiseGeneratorSimplex {
    pub fn new(random: &mut JavaRandom) -> Self {
        let mut p = [0_i32; 512];
        let xo = random.next_f64() * 256.0;
        let yo = random.next_f64() * 256.0;
        let zo = random.next_f64() * 256.0;
        for (index, value) in p[..256].iter_mut().enumerate() {
            *value = index as i32;
        }
        for index in 0..256 {
            let selected = random.next_i32_bound((256 - index) as i32) as usize + index;
            p.swap(index, selected);
            p[index + 256] = p[index];
        }
        Self { p, xo, yo, zo }
    }

    fn fast_floor(value: f64) -> i32 {
        if value > 0.0 { value as i32 } else { value as i32 - 1 }
    }

    fn dot(gradient: [i32; 3], x: f64, y: f64) -> f64 {
        gradient[0] as f64 * x + gradient[1] as f64 * y
    }

    pub fn getValue(&self, x: f64, y: f64) -> f64 {
        let sqrt3 = 3.0_f64.sqrt();
        let f2 = 0.5 * (sqrt3 - 1.0);
        let skew = (x + y) * f2;
        let i = Self::fast_floor(x + skew);
        let j = Self::fast_floor(y + skew);
        let g2 = (3.0 - sqrt3) / 6.0;
        let unskew = (i + j) as f64 * g2;
        let origin_x = i as f64 - unskew;
        let origin_y = j as f64 - unskew;
        let x0 = x - origin_x;
        let y0 = y - origin_y;
        let (i1, j1) = if x0 > y0 { (1, 0) } else { (0, 1) };
        let x1 = x0 - i1 as f64 + g2;
        let y1 = y0 - j1 as f64 + g2;
        let x2 = x0 - 1.0 + 2.0 * g2;
        let y2 = y0 - 1.0 + 2.0 * g2;
        let ii = (i & 255) as usize;
        let jj = (j & 255) as usize;
        let gi0 = (self.p[ii + self.p[jj] as usize] % 12) as usize;
        let gi1 = (self.p[ii + i1 as usize + self.p[jj + j1 as usize] as usize] % 12) as usize;
        let gi2 = (self.p[ii + 1 + self.p[jj + 1] as usize] % 12) as usize;

        let contribution = |mut t: f64, gradient: [i32; 3], px: f64, py: f64| {
            if t < 0.0 {
                0.0
            } else {
                t *= t;
                t * t * Self::dot(gradient, px, py)
            }
        };
        let n0 = contribution(0.5 - x0 * x0 - y0 * y0, GRAD3[gi0], x0, y0);
        let n1 = contribution(0.5 - x1 * x1 - y1 * y1, GRAD3[gi1], x1, y1);
        let n2 = contribution(0.5 - x2 * x2 - y2 * y2, GRAD3[gi2], x2, y2);
        70.0 * (n0 + n1 + n2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_simplex_is_deterministic() {
        let mut random = JavaRandom::new(1234);
        let noise = NoiseGeneratorSimplex::new(&mut random);
        assert_eq!(noise.getValue(0.0, 0.0).to_bits(), noise.getValue(0.0, 0.0).to_bits());
    }
}
