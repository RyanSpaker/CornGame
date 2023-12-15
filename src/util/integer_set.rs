use std::ops::{Sub, Add, SubAssign, AddAssign};

pub trait SubOne{
    /// Returns Self minus one
    fn sub_one(&self) -> Self;
}

/// This struct represents a set of Integers
#[derive(Clone, Debug)]
pub struct IntegerSet<T> 
where T: 
    Clone + 
    PartialOrd + 
    Ord + 
    Default + 
    Sub<Output = T> + 
    Add<Output = T> + 
    SubAssign + AddAssign + SubOne
{
    /// List of endpoints for a set of ranges that represent the set of integers
    endpoints: Vec<T>
}

impl<T> IntegerSet<T> 
where T: 
    Clone + 
    PartialOrd + 
    Ord + 
    Default + 
    Sub<Output = T> + 
    Add<Output = T> + 
    SubAssign + AddAssign + SubOne
{
    /// Returns an Integer set from an overlap vector and a set count by intersection
    pub fn intersect_from_overlap(overlap: Vec<(i32, &T)>, set_count: i32) -> Self{
        let mut state: i32 = 0;
        let mut new_endpoints: Vec<T> = vec![];
        for (state_change, value) in overlap.into_iter(){
            if state < set_count && state + state_change >= set_count{
                new_endpoints.push(value.to_owned());
            }else if state >= set_count && state+state_change < set_count{
                new_endpoints.push(value.to_owned())
            }
            state += state_change;
        }
        return Self{endpoints: new_endpoints};
    }
    /// Returns and Integers set from an overlap vector by union
    pub fn union_from_overlap(overlap: Vec<(i32, &T)>) -> Self{
        let mut state: i32 = 0;
        let mut new_endpoints: Vec<T> = vec![];
        for (state_change, value) in overlap.into_iter(){
            if state < 1 && state + state_change >= 1{
                new_endpoints.push(value.to_owned());
            }else if state >= 1 && state+state_change < 1{
                new_endpoints.push(value.to_owned())
            }
            state += state_change;
        }
        return Self{endpoints: new_endpoints};
    }
    /// Calculates the overlap vector between two Integer sets
    pub fn calculate_overlap<'a>(set_a: &'a Self, set_b: &'a Self) -> Vec<(i32, &'a T)>{
        let mut a_index: usize = 0;
        let mut b_index: usize = 0;
        let mut overlap: Vec<(i32, &'a T)> = vec![];
        while a_index < set_a.endpoints.len() || b_index < set_b.endpoints.len(){
            if b_index >= set_b.endpoints.len(){
                overlap.push(((1-a_index as i32%2)*2-1, &set_a.endpoints[a_index]));
                a_index += 1;
            }else if a_index >= set_a.endpoints.len(){
                overlap.push(((1-b_index as i32%2)*2-1, &set_b.endpoints[b_index]));
                b_index += 1;
            }else if set_a.endpoints[a_index] < set_b.endpoints[b_index]{
                overlap.push(((1-a_index as i32%2)*2-1, &set_a.endpoints[a_index]));
                a_index += 1;
            }else if set_b.endpoints[b_index] < set_a.endpoints[a_index]{
                overlap.push(((1-b_index as i32%2)*2-1, &set_b.endpoints[b_index]));
                b_index += 1;
            }else{
                overlap.push(((1-a_index as i32%2)*2-1 + (1-b_index as i32%2)*2-1, &set_a.endpoints[a_index]));
                a_index += 1;
                b_index += 1;
            }
        }
        return overlap;
    }
    /// Calculates the overlap Vecotr between a list of Integer Sets
    /// 
    /// The overlap Vector is a list containing (state-change, position) pairs, sorted by position
    /// 
    /// If you think of state as being the total number of sets the position is currently residing 
    /// in, then as you walk along the overlap vector, you can add the state-change part of the 
    /// pair to calculate how many sets each position is part of. This is useful for calculating 
    /// the union and intersection of an arbitrary number of sets. 
    /// 
    /// Union would be any position in which the state changes from 0-> >1 or from >1 -> 0
    /// 
    /// Intersection would be any position in which the state changes from <#sets -> #sets or #sets -> <#sets
    pub fn calculate_overlap_vec<'a>(sets: &'a Vec<Self>) -> Vec<(i32, &'a T)>{
        let mut indices: Vec<usize> = vec![0; sets.len()];
        let mut overlap: Vec<(i32, &'a T)> = vec![];
        while indices.iter().enumerate().any(|(i, index)| *index < sets[i].endpoints.len()){
            let valid_indices: Vec<usize> = indices.iter().enumerate().filter_map(|(i, index)|{
                if *index >= sets[i].endpoints.len() {return None;}
                return Some(i);
            }).collect();
            let mut min: &'a T = &sets[valid_indices[0]].endpoints[indices[valid_indices[0]]];
            let mut mindices: Vec<usize> = vec![];
            valid_indices.into_iter().for_each(|index| {
                if sets[index].endpoints[indices[index]] < *min{
                    min = &sets[index].endpoints[indices[index]];
                    mindices = vec![index];
                }else if sets[index].endpoints[indices[index]] == *min {
                    mindices.push(index);
                }
            });
            let state_change = mindices.into_iter().map(|index| {
                let map = (1-indices[index] as i32%2)*2-1;
                indices[index] += 1;
                return map;
            }).sum();
            overlap.push((state_change, min));
        }
        return overlap;
    }
    /// Creates a set from the intersection of two sets
    pub fn intersection(set_a: &Self, set_b: &Self) -> Self{
        return Self::intersect_from_overlap(Self::calculate_overlap(set_a, set_b), 2);
    }
    /// Creates a set from the intersection of a list of sets
    pub fn intersect_all(sets: &Vec<Self>) -> Self{
        return Self::intersect_from_overlap(Self::calculate_overlap_vec(sets), sets.len() as i32);
    }
    /// Mutates self by intersecting it with another set
    pub fn intersect_with(&mut self, rhs: &Self) -> &mut Self{
        let overlap = Self::calculate_overlap(self, rhs);
        let mut state: i32 = 0;
        let mut new_endpoints: Vec<T> = vec![];
        for (state_change, value) in overlap.into_iter(){
            if state < 2 && state + state_change >= 2{
                new_endpoints.push(value.to_owned());
            }else if state >= 2 && state+state_change < 2{
                new_endpoints.push(value.to_owned())
            }
            state += state_change;
        }
        self.endpoints = new_endpoints;
        return self;
    }
    /// Creates a set from the union of two sets
    pub fn union(set_a: &Self, set_b: &Self) -> Self{
        return Self::union_from_overlap(Self::calculate_overlap(set_a, set_b));
    }
    /// Creates a set form the union of a list of sets
    pub fn union_all(sets: &Vec<Self>) -> Self{
        return Self::union_from_overlap(Self::calculate_overlap_vec(sets));
    }
    /// Mutates self by taking the union of it and another set
    pub fn union_with(&mut self, rhs: &Self) -> &mut Self{
        let overlap = Self::calculate_overlap(self, rhs);
        let mut state: i32 = 0;
        let mut new_endpoints: Vec<T> = vec![];
        for (state_change, value) in overlap.into_iter(){
            if state < 1 && state + state_change >= 1{
                new_endpoints.push(value.to_owned());
            }else if state >= 1 && state+state_change < 1{
                new_endpoints.push(value.to_owned())
            }
            state += state_change;
        }
        self.endpoints = new_endpoints;
        return self;
    }
    /// Mutates self by taking its complement, or inverse. Min and Max represent the Domain [) to use for the operation
    pub fn complement(&mut self, min: T, max: T) -> &mut Self{
        if self.endpoints.len() == 0 {
            self.endpoints = vec![min, max];
            return self;
        }
        let cur_min = &self.min().unwrap();
        let cur_end = &self.end().unwrap();
        self.endpoints.remove(0); self.endpoints.pop();
        self.intersect_with(&Self::simple(&min, &max));
        if *cur_min > min {
            self.endpoints.insert(0, cur_min.to_owned());
            self.endpoints.insert(0, min);
        }
        if *cur_end < max{
            self.endpoints.push(cur_end.to_owned());
            self.endpoints.push(max);
        }
        return self;
    }
    /// Returns the complement of set using min and max as the domain [)
    pub fn not(set: &Self, min: T, max: T) -> Self{
        if set.endpoints.len() == 0 {
            return Self::simple(&min, &max);
        }
        let cur_min = &set.min().unwrap();
        let cur_end = &set.end().unwrap();
        let mut result = set.to_owned();
        result.endpoints.remove(0); result.endpoints.pop();
        result.intersect_with(&Self::simple(&min, &max));
        if *cur_min > min {
            result.endpoints.insert(0, cur_min.to_owned());
            result.endpoints.insert(0, min);
        }
        if *cur_end < max{
            result.endpoints.push(cur_end.to_owned());
            result.endpoints.push(max);
        }
        return result;
    }
    /// Mutates self by taking its difference with another set
    pub fn difference_with(&mut self, rhs: &Self) -> &mut Self{
        if self.is_empty() {return self;}
        return self.intersect_with(&Self::not(rhs, self.min().unwrap(), self.end().unwrap()));
    }
    /// Creates a set from the difference of set and rhs
    pub fn difference(set: &Self, rhs: &Self) -> Self{
        if set.is_empty() {return set.to_owned();}
        return Self::intersection(set, &Self::not(rhs, set.min().unwrap(), set.end().unwrap()))
    }
    /// Returns the minimum value in the set, if the set isn't empty
    pub fn min(&self) -> Option<T>{
        self.endpoints.get(0).cloned()
    }
    /// Returns the last element in the set, if the set isn't empty
    pub fn max(&self) -> Option<T>{
        if self.endpoints.len() == 0 {return None;}
        return Some(self.endpoints.last().unwrap().to_owned().sub_one());
    }
    /// Returns one higher than the last element in the set, if the set isn't empty.
    pub fn end(&self) -> Option<T>{
        return self.endpoints.last().cloned();
    }
    /// Adds a range [) of Integers to the set, 
    pub fn add_range(&mut self, start: T, end: T) -> &mut Self{
        assert!(end > start, "Tried adding invalid range");
        return self.union_with(&Self::simple(&start, &end));
    }
    /// Returns the total number of Integers in the set
    pub fn len(&self) -> T{
        if self.endpoints.len() == 0 {return T::default();}
        let mut count: T = T::default();
        for i in 0..self.endpoints.len()/2{
            count = count + (self.endpoints[i*2+1].to_owned()-self.endpoints[i*2].to_owned());
        }
        count
    }
    /// Returns whether rhs resides entirely within self
    pub fn contains(&self, rhs: &Self) -> bool{
        Self::intersection(self, rhs).len() == rhs.len()
    }
    /// Creates an Integer set from a single range [)
    pub fn simple(start: &T, end: &T) -> Self{
        assert!(start!=end, "Cant create bufferrange with start and end being equal");
        return Self{endpoints: vec![start.to_owned(), end.to_owned()]};
    }
    /// Returns an Integer set of length continuos elements from self, returning None if there is no continuos range of that size in self
    pub fn get_continuos(&self, length: T) -> Option<Self>{
        let mut found = false;
        let mut min = T::default();
        let mut mindex = 0;
        self.endpoints.to_owned().chunks(2).enumerate().for_each(|(i, range)| {
            let range_length: T = range[2].to_owned() - range[1].to_owned();
            if range_length >= length{
                if !found || min > range_length{
                    min = range_length;
                    mindex = i*2;
                }
                found = true;
            }
        });
        if !found {return None;}
        return Some(Self::simple(&self.endpoints[mindex], &(self.endpoints[mindex].to_owned()+length)));
    }
    /// Returns and endpoint at a specific index
    pub fn get_endpoint(&self, index: usize) -> T{
        return self.endpoints[index].to_owned();
    }
    /// Returns the total number of endpoints, =2*range count
    pub fn endpoint_count(&self) -> usize{
        return self.endpoints.len();
    }
    /// Takes count integers from self, returning them as a new Integer set, also returns the total number of items unable to be taken if count is greater than the len of self
    pub fn take(&mut self, mut count: T) -> (Self, T){
        let mut new_endpoints = vec![];
        while self.len() > T::default() && count > T::default(){
            if self.endpoints[1].to_owned() - self.endpoints[0].to_owned() <= count{
                count -= self.endpoints[1].to_owned() - self.endpoints[0].to_owned();
                new_endpoints.push(self.endpoints.remove(0));
                new_endpoints.push(self.endpoints.remove(0));
            }else{
                new_endpoints.push(self.endpoints[0].to_owned());
                new_endpoints.push(self.endpoints[0].to_owned() + count.to_owned());
                self.endpoints[0] += count.to_owned();
                count = T::default();
            }
        }
        return (Self{endpoints: new_endpoints}, count);
    }
    /// Returns whether the set has no elements in it
    pub fn is_empty(&self) -> bool{
        self.endpoints.len() == 0
    }
    /// Returns the total number of distinct ranges in the Set
    pub fn range_count(&self) -> usize{
        return self.endpoints.len()/2;
    }
    /// Returns a list of continuos ranges that make up the set: Vec<(start, end)>
    pub fn get_continuos_ranges(&self) -> Vec<(T, T)>{
        self.endpoints.chunks(2).map(|chunk| (chunk[0].to_owned(), chunk[1].to_owned())).collect()
    }
}

impl<T> Default for IntegerSet<T> 
where T: 
    Clone + 
    PartialOrd + 
    Ord + 
    Default + 
    Sub<Output = T> + 
    Add<Output = T> + 
    SubAssign + AddAssign + SubOne
{
    /// Defaults to an empty set
    fn default() -> Self {
        Self{endpoints: vec![]}
    }
}
