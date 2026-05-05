use js_sys::{Array, BigUint64Array, Math};
use std::{collections::BTreeMap, hash::Hash};

use num_bigint::{BigInt, Sign};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct ZDD {
    inner: ZDDInner,
}

#[wasm_bindgen]
impl ZDD {
    #[wasm_bindgen(constructor)]
    pub fn new() -> ZDD {
        ZDD {
            inner: ZDDInner::new(),
        }
    }

    pub fn zero(&self) -> NodeId {
        ZDDInner::zero()
    }

    pub fn one(&self) -> NodeId {
        ZDDInner::one()
    }

    pub fn empty_set(&self) -> NodeId {
        ZDDInner::empty_set()
    }

    pub fn singleton(&mut self, var_id: VarId) -> NodeId {
        self.inner.singleton(var_id)
    }

    pub fn union(&mut self, a: NodeId, b: NodeId) -> Result<NodeId, JsValue> {
        self.validate_node(a)?;
        self.validate_node(b)?;

        Ok(self.inner.apply(Op::Union, a, b))
    }

    pub fn intersect(&mut self, a: NodeId, b: NodeId) -> Result<NodeId, JsValue> {
        self.validate_node(a)?;
        self.validate_node(b)?;

        Ok(self.inner.apply(Op::Intersect, a, b))
    }

    pub fn diff(&mut self, a: NodeId, b: NodeId) -> Result<NodeId, JsValue> {
        self.validate_node(a)?;
        self.validate_node(b)?;

        Ok(self.inner.apply(Op::Diff, a, b))
    }

    pub fn product(&mut self, a: NodeId, b: NodeId) -> Result<NodeId, JsValue> {
        self.validate_node(a)?;
        self.validate_node(b)?;

        Ok(self.inner.product(a, b))
    }

    pub fn add_var(&mut self, var_id: VarId, family: NodeId) -> Result<NodeId, JsValue> {
        self.validate_node(family)?;

        if family >= 2 {
            let head = self.inner.var_of(family);

            if var_id >= head {
                return Err(JsValue::from_str(
                    "invalid variable order: var_id must be smaller than the first variable of family",
                ));
            }
        }

        Ok(self.inner.add_var(var_id, family))
    }

    pub fn count(&self, root: NodeId) -> Result<String, JsValue> {
        self.validate_node(root)?;

        Ok(self.inner.count(root).to_string())
    }

    pub fn enumerate(&self, root: NodeId) -> Result<JsValue, JsValue> {
        self.validate_node(root)?;

        let family = self
            .inner
            .enumerate(root)
            .iter()
            .map(|f| f.iter().map(|f| *f as u64).collect::<Vec<u64>>())
            .collect::<Vec<Vec<u64>>>();

        let out = Array::new();

        for row in family {
            let typed = BigUint64Array::new_from_slice(&row);
            out.push(&typed.into());
        }

        Ok(out.into())
    }

    pub fn sample(&self, root: NodeId) -> Result<JsValue, JsValue> {
        self.validate_node(root)?;

        let set = self
            .inner
            .sample(root)
            .ok_or_else(|| JsValue::from_str("cannot sample from the empty family"))?;
        let row = set.iter().map(|v| *v as u64).collect::<Vec<u64>>();

        Ok(BigUint64Array::new_from_slice(&row).into())
    }

    pub fn node_count(&self) -> usize {
        self.inner.nodes.len()
    }

    pub fn real_node_count(&self) -> usize {
        self.inner.nodes.len().saturating_sub(2)
    }

    pub fn clear_apply_cache(&mut self) {
        self.inner.apply_cache.clear();
    }

    fn validate_node(&self, id: NodeId) -> Result<(), JsValue> {
        if id < self.inner.nodes.len() {
            Ok(())
        } else {
            Err(JsValue::from_str(&format!("invalid NodeId: {}", id)))
        }
    }
}

pub type NodeId = usize;
pub type VarId = usize;

#[derive(Clone, Copy)]
pub struct Node {
    var: VarId,
    lo: NodeId,
    hi: NodeId,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Op {
    Union,
    Intersect,
    Diff,
}
struct ZDDInner {
    nodes: Vec<Node>,
    unique: BTreeMap<(VarId, NodeId, NodeId), NodeId>,
    apply_cache: BTreeMap<(Op, NodeId, NodeId), NodeId>,
}
impl ZDDInner {
    pub fn new() -> Self {
        Self {
            nodes: vec![
                Node {
                    var: VarId::MAX,
                    lo: 0,
                    hi: 0,
                },
                Node {
                    var: VarId::MAX,
                    lo: 1,
                    hi: 1,
                },
            ],
            unique: BTreeMap::new(),
            apply_cache: BTreeMap::new(),
        }
    }
    pub fn zero() -> NodeId {
        return 0;
    }
    pub fn one() -> NodeId {
        return 1;
    }
    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id]
    }
    fn make(&mut self, var_id: VarId, lo: NodeId, hi: NodeId) -> NodeId {
        if hi == 0 {
            return lo;
        };
        let val = self.unique.get(&(var_id, lo, hi));
        if let Some(val) = val {
            return *val;
        };
        let id = self.nodes.len() as NodeId;
        self.nodes.push(Node {
            var: var_id,
            lo,
            hi,
        });
        self.unique.insert((var_id, lo, hi), id);
        id
    }
    pub fn singleton(&mut self, var_id: VarId) -> NodeId {
        self.make(var_id, 0, 1)
    }
    pub fn empty_set() -> NodeId {
        1
    }
    fn var_of(&self, id: NodeId) -> VarId {
        if id < 2 {
            VarId::MAX
        } else {
            self.nodes[id].var
        }
    }
    fn lo(&self, id: NodeId, v: VarId) -> NodeId {
        if id < 2 {
            id
        } else {
            let n = self.nodes[id];
            if n.var == v { n.lo } else { id }
        }
    }
    fn hi(&self, id: NodeId, v: VarId) -> NodeId {
        if id < 2 {
            0
        } else {
            let n = self.nodes[id];
            if n.var == v { n.hi } else { 0 }
        }
    }
    pub fn apply(&mut self, op: Op, a: NodeId, b: NodeId) -> NodeId {
        let cached = self.apply_cache.get(&(op, a, b));
        if let Some(cached) = cached {
            return *cached;
        };
        let terminal = Self::apply_terminal(op, a, b);
        if let Some(terminal) = terminal {
            self.apply_cache.insert((op, a, b), terminal);
            return terminal;
        };
        let v = VarId::min(self.var_of(a), self.var_of(b));
        let a0 = self.lo(a, v);
        let a1 = self.hi(a, v);
        let b0 = self.lo(b, v);
        let b1 = self.hi(b, v);
        let lo = self.apply(op, a0, b0);
        let hi = self.apply(op, a1, b1);
        let result = self.make(v, lo, hi);
        self.apply_cache.insert((op, a, b), result);
        result
    }
    fn apply_terminal(op: Op, a: NodeId, b: NodeId) -> Option<NodeId> {
        match op {
            Op::Union => {
                if a == 0 {
                    return Some(b);
                };
                if b == 0 {
                    return Some(a);
                };
                if a == b {
                    return Some(a);
                };
                return None;
            }
            Op::Intersect => {
                if a == 0 || b == 0 {
                    return Some(0);
                };
                if a == b {
                    return Some(a);
                };
                if a == 1 && b == 1 {
                    return Some(1);
                };
                return None;
            }
            Op::Diff => {
                if a == 0 {
                    return Some(0);
                };
                if b == 0 {
                    return Some(a);
                };
                if a == b {
                    return Some(0);
                };
                return None;
            }
        }
    }
    pub fn add_var(&mut self, var_id: VarId, family: NodeId) -> NodeId {
        self.make(var_id, 0, family)
    }
    fn product_reccursice(
        &mut self,
        cache: &mut BTreeMap<(NodeId, NodeId), NodeId>,
        x: NodeId,
        y: NodeId,
    ) -> NodeId {
        if x == 0 || y == 0 {
            return 0;
        };
        if x == 1 {
            return y;
        };
        if y == 1 {
            return x;
        };
        let cached = cache.get(&(x, y));
        if let Some(cached) = cached {
            return *cached;
        };
        let vx = self.var_of(x);
        let vy = self.var_of(y);
        let v = VarId::min(vx, vy);
        let x0 = self.lo(x, v);
        let x1 = self.hi(x, v);
        let y0 = self.lo(y, v);
        let y1 = self.hi(y, v);
        let lo = self.product_reccursice(cache, x0, y0);
        let x1y0 = self.product_reccursice(cache, x1, y0);
        let x0y1 = self.product_reccursice(cache, x0, y1);
        let x1y1 = self.product_reccursice(cache, x1, y1);
        let hi_left = self.apply(Op::Union, x1y0, x0y1);
        let hi = self.apply(Op::Union, hi_left, x1y1);
        let result = self.make(v, lo, hi);
        cache.insert((x, y), result);
        result
    }
    pub fn product(&mut self, a: NodeId, b: NodeId) -> NodeId {
        let mut cache: BTreeMap<(NodeId, NodeId), NodeId> = BTreeMap::new();
        self.product_reccursice(&mut cache, a, b)
    }
    pub fn count_reccursive(&self, cache: &mut BTreeMap<NodeId, BigInt>, id: NodeId) -> BigInt {
        if id == 0 {
            return BigInt::from(0);
        };
        if id == 1 {
            return BigInt::from(1);
        };
        let cached = cache.get(&id);
        if let Some(cached) = cached {
            return cached.clone();
        };
        let n = self.nodes[id];
        let result = self.count_reccursive(cache, n.lo);
        let result2 = self.count_reccursive(cache, n.hi);
        let res = result + result2;
        cache.insert(id, res.clone());
        res
    }
    pub fn count(&self, root: NodeId) -> BigInt {
        let mut cache: BTreeMap<NodeId, BigInt> = BTreeMap::new();
        self.count_reccursive(&mut cache, root)
    }
    fn bit_len(value: &BigInt) -> usize {
        let (_, bytes) = value.to_bytes_be();
        if bytes.is_empty() {
            return 0;
        }

        (bytes.len() - 1) * 8 + (8 - bytes[0].leading_zeros() as usize)
    }
    fn random_bigint_less_than(upper: &BigInt) -> BigInt {
        let bit_len = Self::bit_len(upper);
        let byte_len = bit_len.div_ceil(8);
        let excess_bits = byte_len * 8 - bit_len;

        loop {
            let mut bytes = (0..byte_len)
                .map(|_| (Math::random() * 256.0).floor() as u8)
                .collect::<Vec<u8>>();

            if excess_bits > 0 {
                bytes[0] &= 0xff >> excess_bits;
            }

            let candidate = BigInt::from_bytes_be(Sign::Plus, &bytes);
            if candidate < *upper {
                return candidate;
            }
        }
    }
    fn set_at_index_reccursive(
        &self,
        cache: &mut BTreeMap<NodeId, BigInt>,
        id: NodeId,
        index: BigInt,
        acc: &mut Vec<VarId>,
    ) -> bool {
        if id == 0 {
            return false;
        }
        if id == 1 {
            return index == BigInt::from(0);
        }

        let n = self.nodes[id];
        let lo_count = self.count_reccursive(cache, n.lo);

        if index < lo_count {
            return self.set_at_index_reccursive(cache, n.lo, index, acc);
        }

        acc.push(n.var);
        let found = self.set_at_index_reccursive(cache, n.hi, index - lo_count, acc);
        if !found {
            acc.pop();
        }
        found
    }
    #[cfg(test)]
    fn set_at_index(&self, root: NodeId, index: BigInt) -> Option<Vec<VarId>> {
        let mut cache: BTreeMap<NodeId, BigInt> = BTreeMap::new();
        let total = self.count_reccursive(&mut cache, root);
        if index < BigInt::from(0) || index >= total {
            return None;
        }

        let mut result = vec![];
        if self.set_at_index_reccursive(&mut cache, root, index, &mut result) {
            Some(result)
        } else {
            None
        }
    }
    pub fn sample(&self, root: NodeId) -> Option<Vec<VarId>> {
        let mut cache: BTreeMap<NodeId, BigInt> = BTreeMap::new();
        let total = self.count_reccursive(&mut cache, root);
        if total == BigInt::from(0) {
            return None;
        }

        let index = Self::random_bigint_less_than(&total);
        let mut result = vec![];
        if self.set_at_index_reccursive(&mut cache, root, index, &mut result) {
            Some(result)
        } else {
            None
        }
    }
    fn enumerate_reccursive(&self, result: &mut Vec<Vec<VarId>>, id: NodeId, acc: &mut Vec<VarId>) {
        if id == 0 {
            return;
        }
        if id == 1 {
            result.push(acc.clone());
            return;
        }
        let n = self.nodes[id];
        self.enumerate_reccursive(result, n.lo, acc);
        acc.push(n.var);
        self.enumerate_reccursive(result, n.hi, acc);
        acc.pop();
    }
    pub fn enumerate(&self, root: NodeId) -> Vec<Vec<VarId>> {
        let mut result: Vec<Vec<VarId>> = vec![];
        self.enumerate_reccursive(&mut result, root, &mut vec![]);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn set_at_index_matches_enumerate_order() {
        let mut zdd = ZDDInner::new();
        let a = zdd.singleton(1);
        let b = zdd.singleton(2);
        let ab = zdd.product(a, b);
        let non_empty = zdd.apply(Op::Union, a, ab);
        let root = zdd.apply(Op::Union, ZDDInner::empty_set(), non_empty);

        assert_eq!(zdd.enumerate(root), vec![vec![], vec![1], vec![1, 2]]);
        assert_eq!(zdd.set_at_index(root, BigInt::from(0)), Some(vec![]));
        assert_eq!(zdd.set_at_index(root, BigInt::from(1)), Some(vec![1]));
        assert_eq!(zdd.set_at_index(root, BigInt::from(2)), Some(vec![1, 2]));
        assert_eq!(zdd.set_at_index(root, BigInt::from(3)), None);
    }
}
