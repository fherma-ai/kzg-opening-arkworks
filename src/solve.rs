//! A KZG opening: the value at `z`, and a commitment to the quotient.
//!
//! Two halves. The quotient comes from synthetic division, which is `N`
//! multiply-adds and gives `p(z)` as its remainder — so the value is free and
//! is not computed a second time. Then one multi-scalar multiplication, which
//! is the rest of the cost.
use ark_bn254::{Fq, Fr, G1Affine, G1Projective};
use ark_ec::{AffineRepr, CurveGroup, VariableBaseMSM};
use ark_ff::{BigInt, PrimeField, Zero};

use crate::fherma::{Inputs, Outputs, Point, Tensor};

fn fe<F: PrimeField<BigInt = BigInt<4>>>(data: &[u64], i: usize, limbs: usize) -> F {
    let mut words = [0u64; 4];
    words[..limbs].copy_from_slice(&data[i * limbs..(i + 1) * limbs]);
    F::from_bigint(BigInt::new(words)).expect("a field element below the modulus")
}

fn row<F: PrimeField<BigInt = BigInt<4>>>(value: F, limbs: usize) -> Tensor<u64> {
    Tensor { shape: vec![limbs as i64], data: value.into_bigint().0[..limbs].to_vec() }
}

#[derive(Default)]
pub struct State {
    limbs: usize,
    /// The reference string, decoded once: it is the point's, not the case's.
    bases: Vec<G1Affine>,
}

pub fn init(p: &Point) -> State {
    let limbs = p.L as usize;
    let n = p.srs_x.data.len() / limbs;
    let bases = (0..n)
        .map(|i| {
            G1Affine::new_unchecked(
                fe::<Fq>(&p.srs_x.data, i, limbs),
                fe::<Fq>(&p.srs_y.data, i, limbs),
            )
        })
        .collect();
    State { limbs, bases }
}

pub fn run(state: &mut State, inp: &Inputs) -> Outputs {
    let limbs = state.limbs;
    let n = inp.coeffs.data.len() / limbs;
    let z: Fr = fe(&inp.z.data, 0, limbs);

    // Synthetic division from the top coefficient down. What falls out at the
    // bottom is the remainder, which for a divisor of X - z is p(z) — Horner
    // and the quotient are the same sweep, so neither is paid for twice.
    let mut quotient = vec![Fr::zero(); n.saturating_sub(1)];
    let mut carry = Fr::zero();
    for i in (0..n).rev() {
        let c: Fr = fe(&inp.coeffs.data, i, limbs);
        if i > 0 {
            quotient[i - 1] = carry;
        }
        carry = c + carry * z;
    }
    let value = carry;

    let commitment = G1Projective::msm(&state.bases[..quotient.len()], &quotient)
        .expect("one quotient coefficient for every point used")
        .into_affine();
    let (x, y) = commitment.xy().unwrap_or((Fq::zero(), Fq::zero()));

    Outputs { px: row(x, limbs), py: row(y, limbs), value: row(value, limbs) }
}

pub fn free(state: State) {
    drop(state);
}
