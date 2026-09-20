//! Native queue allocation control for world_command_storage.py (Linux/glibc).
//! Queue entries carry two weak state references and an identity, like PyO3,
//! but own no payloads. Frame counts are positional command-line arguments.

use std::{hint::black_box, sync::Arc};

use bevy_ecs::world::World;
use libc::mallinfo2;

fn allocated() -> usize {
    // SAFETY: glibc returns allocator statistics by value and accepts no pointers.
    let info = unsafe { mallinfo2() };
    info.uordblks + info.hblkhd
}

fn main() {
    let mut world = World::new();
    let pending = Arc::new(0_u64);
    let errors = Arc::new(0_u64);
    let counts = std::env::args()
        .skip(1)
        .map(|count| count.parse::<u64>().expect("nonnegative frame count"))
        .collect::<Vec<_>>();
    let start = allocated();
    println!("{{\"stage\":\"start\",\"allocated\":{start}}}");
    for count in counts {
        for id in 0..count {
            let pending = Arc::downgrade(&pending);
            let errors = Arc::downgrade(&errors);
            world.commands().queue(move |_: &mut World| {
                black_box((pending.upgrade(), errors.upgrade(), id));
            });
        }
        let queued = allocated();
        world.flush();
        let flushed = allocated();
        println!("{{\"count\":{count},\"pending\":{queued},\"flushed\":{flushed}}}");
    }
    drop(world);
    println!("{{\"stage\":\"dropped\",\"allocated\":{}}}", allocated());
}
