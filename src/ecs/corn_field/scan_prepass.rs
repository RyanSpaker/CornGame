use bevy::render::{render_resource::{Buffer, BufferDescriptor, BufferUsages}, renderer::RenderDevice};

#[derive(Default)]
pub struct VoteScanCompactBuffers{
    vote_scan: Option<Buffer>,
    count_buffers: Option<(Buffer, Buffer)>,
    count_sizes: (u64, u64),
    index_buffer: Option<Buffer>,
    indirect_buffer: Option<Buffer>,
    lod_count: usize
}
impl VoteScanCompactBuffers{
    pub fn init(&mut self, render_device: &RenderDevice, lods: usize, instance_count: u64){
        self.lod_count = lods;
        self.vote_scan = Some(render_device.create_buffer(&BufferDescriptor{ 
            label: Some("Vote Scan Buffer"), 
            size: instance_count * 8u64, 
            usage: BufferUsages::STORAGE, 
            mapped_at_creation: false
        }));
        self.index_buffer = Some(render_device.create_buffer(&BufferDescriptor{ 
            label: Some("Corn Index Buffer"), 
            size: instance_count * 4u64, 
            usage: BufferUsages::STORAGE, 
            mapped_at_creation: false
        }));
        self.indirect_buffer = Some(render_device.create_buffer(&BufferDescriptor{ 
            label: Some("Corn Indirect Buffer"), 
            size: lods as u64 * 20u64, 
            usage: BufferUsages::STORAGE | BufferUsages::INDIRECT, 
            mapped_at_creation: false
        }));
        self.count_sizes = (instance_count/256+1, (instance_count/256+1)/256+1);
        self.count_buffers = Some((
            render_device.create_buffer(&BufferDescriptor{ 
                label: Some("Intermediate Count Buffer 1"), 
                size: self.count_sizes.0 * 4u64*(lods+1) as u64, 
                usage: BufferUsages::STORAGE, 
                mapped_at_creation: false
            }),
            render_device.create_buffer(&BufferDescriptor{ 
                label: Some("Intermediate Count Buffer 2"), 
                size: self.count_sizes.1 * 4u64*(lods+1) as u64, 
                usage: BufferUsages::STORAGE, 
                mapped_at_creation: false
            })
        ));
    }
    pub fn update_size(&mut self, render_device: &RenderDevice, instance_count: u64){
        self.vote_scan = Some(render_device.create_buffer(&BufferDescriptor{ 
            label: Some("Vote Scan Buffer"), 
            size: instance_count * 8u64, 
            usage: BufferUsages::STORAGE, 
            mapped_at_creation: false
        }));
        self.index_buffer = Some(render_device.create_buffer(&BufferDescriptor{ 
            label: Some("Corn Index Buffer"), 
            size: instance_count * 4u64, 
            usage: BufferUsages::STORAGE, 
            mapped_at_creation: false
        }));
        self.count_sizes = (instance_count/256+1, (instance_count/256+1)/256+1);
        self.count_buffers = Some((
            render_device.create_buffer(&BufferDescriptor{ 
                label: Some("Intermediate Count Buffer 1"), 
                size: self.count_sizes.0 * 4u64*(self.lod_count+1) as u64, 
                usage: BufferUsages::STORAGE, 
                mapped_at_creation: false
            }),
            render_device.create_buffer(&BufferDescriptor{ 
                label: Some("Intermediate Count Buffer 2"), 
                size: self.count_sizes.1 * 4u64*(self.lod_count+1) as u64, 
                usage: BufferUsages::STORAGE, 
                mapped_at_creation: false
            })
        ));
    }
}