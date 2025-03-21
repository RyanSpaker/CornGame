use bevy::{prelude::*, reflect::Reflect, utils::hashbrown::HashMap};
use super::entity::LoadEntityName;

/// A systemset that runs when a node of the LoadingQuadTree has is_loading=true \
/// Automatically parented to parent node sets when LoadingQuadTree is added to the app.
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, SystemSet)]
pub struct QuadTreeNodeLoadingSet(pub usize);
/// A systemset that runs when a node of the LoadingQuadTree has is_unloading=true \
/// Automatically parented to parent node sets when LoadingQuadTree is added to the app.
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, SystemSet)]
pub struct QuadTreeNodeUnloadingSet(pub usize);

/// A quad tree representing the loading status of all LoadingTasks
#[derive(Debug, Default, Clone, PartialEq, Eq, Reflect, Resource)]
pub struct LoadingQuadTree{
    // Internal tree store
    data: Vec<LoadingQuadTreeNode>,
    // Map from LoadEntityName, to the node representing it
    name_map: HashMap<LoadEntityName, usize>,
    next_free: usize,
    child_count: usize
}
impl LoadingQuadTree{
    pub fn get_depth_sub_index(&self, mut index: usize) -> (usize, usize){
        let mut depth: u32 = 0;
        while self.child_count.pow(depth)<=index {index -= self.child_count.pow(depth); depth +=1;}
        (depth as usize, index)
    }
    pub fn get_nth_child(&self, index: usize, n: usize) -> usize{
        // Get depth of index, and the sub_index, index relative to the current layer
        let (depth, sub_index) = self.get_depth_sub_index(index);
        // Start of the child layer. get remainder of current layer, + current index
        let child_level_offset = self.child_count.pow(depth as u32)-sub_index+index;
        // Start of child layer + new sub_index = new index
        child_level_offset+sub_index*self.child_count+n
    }
    pub fn get_parent(&self, index: usize) -> usize{
        let (depth, sub_index) = self.get_depth_sub_index(index);
        // Dividing by 4 negates the sub_index*4+n, when calculating a child index.
        let parent_sub_index = sub_index/self.child_count;
        // index-sub_index = start of current layer, -4^depth-1 = start of previous layer, +psl = parent index
        index-sub_index-self.child_count.pow(depth as u32-1)+parent_sub_index
    }
    pub fn is_loading_criteria(index: usize) -> impl FnMut(Res<Self>)->bool{
        move |res: Res<Self>| {
            res.data[index].is_loading
        }
    }
    pub fn is_unloading_criteria(index: usize) -> impl FnMut(Res<Self>)->bool{
        move |res: Res<Self>| {
            res.data[index].is_unloading
        }
    }
    pub fn name_is_loading_criteria(name: LoadEntityName) -> impl FnMut(Res<Self>)->bool{
        move |res: Res<Self>| {
            if let Some(index) = res.name_map.get(&name) {
                res.data[*index as usize].is_loading
            }else {false}
        }
    }
    pub fn name_is_unloading_criteria(name: LoadEntityName) -> impl FnMut(Res<Self>)->bool{
        move |res: Res<Self>| {
            if let Some(index) = res.name_map.get(&name) {
                res.data[*index as usize].is_unloading
            }else {false}
        }
    }
    pub fn initialize(app: &mut App, child_count: usize, depth: usize){
        // sum of 4^i for 0<=i<=depth-1
        let data_size = (child_count.pow(depth as u32)-1)/3;
        let res = Self{data: Vec::with_capacity(data_size as usize), name_map: HashMap::default(), next_free: 0, child_count};
        // Setup system sets
        app.configure_sets(Update, QuadTreeNodeLoadingSet(0).run_if(Self::is_loading_criteria(0)));
        app.configure_sets(Update, QuadTreeNodeUnloadingSet(0).run_if(Self::is_unloading_criteria(0)));
        for i in 1..data_size as usize{
            app.configure_sets(Update, (
                QuadTreeNodeLoadingSet(i).run_if(Self::is_loading_criteria(i)).in_set(QuadTreeNodeLoadingSet(res.get_parent(i))),
                QuadTreeNodeUnloadingSet(i).run_if(Self::is_unloading_criteria(i)).in_set(QuadTreeNodeUnloadingSet(res.get_parent(i))),
            ));
        }
        app.insert_resource(res);
    }
    pub fn insert(&mut self, name: LoadEntityName){
        // Turn the parent node into a leaf if necessary
        if self.next_free%self.child_count==0 && self.next_free>0 {
            let parent_index = self.get_parent(self.next_free);
            self.data.push(self.data[parent_index].clone());
            let Some(name) = self.data[parent_index].entity_name.take() else {
                panic!("LoadingQuadTree, Node became a parent, but wasnt a leaf beforehand.");
            };
            self.name_map.insert(name, self.next_free);
            self.next_free += 1;
        }
        // Add new node
        self.name_map.insert(name.clone(), self.next_free);
        self.data.push(LoadingQuadTreeNode{entity_name: Some(name), is_loading: false, is_unloading: false});
        self.next_free += 1;
    }
    pub fn remove(&mut self, name: LoadEntityName){
        let Some(index) = self.name_map.remove(&name) else {return;};
        // Either just remove the node if it is the last, otherwise swap the last node and this one.
        if index==self.next_free-1{
            self.data.pop();
            self.next_free -= 1;
        } else {
            let Some(leaf) = self.data.pop() else {panic!("Remove node from tree, but tree was empty");};
            let Some(name) = leaf.entity_name.clone() else {panic!("Last node in tree was not a leaf");};
            if let Some(i) = self.name_map.get_mut(&name) {*i=index;}
            self.data[index] = leaf;
            self.next_free -= 1;
        }
        // Make sure leaves that can move up a layer do so
        if self.next_free%self.child_count==1 {
            let parent_index = self.get_parent(self.next_free-1);
            self.data[parent_index] = self.data.pop().unwrap();
            self.next_free -= 1;
            let Some(name) = self.data[parent_index].entity_name.clone() else {panic!("Last node in tree was not a leaf. (2nd)");};
            if let Some(i) = self.name_map.get_mut(&name) {
                *i = parent_index;
            }
        }
    }
}

/// Nodes of the LoadingQuadTree
#[derive(Debug, Default, Clone, PartialEq, Eq, Reflect)]
pub struct LoadingQuadTreeNode{
    /// Name of the load entity this node refers to if it is a leaf
    pub entity_name: Option<LoadEntityName>,
    /// Whether this node, or any child nodes are loading
    pub is_loading: bool,
    /// Whether this node, or any child nodes are unloading
    pub is_unloading: bool
}