struct Dimensions { width:u32, height:u32, depth:u32, count:u32 }
@group(0) @binding(0) var<storage,read> source:array<u32>;
@group(0) @binding(1) var<storage,read_write> output_heat:array<u32>;
@group(0) @binding(2) var<storage,read_write> status:atomic<u32>;
@group(0) @binding(3) var<uniform> dimensions:Dimensions;

fn neighbor(position:u32,direction:u32)->u32 {
    var x=position%dimensions.width;
    var y=(position/dimensions.width)%dimensions.height;
    var z=position/(dimensions.width*dimensions.height);
    switch direction {
        case 0u: { x=(x+1u)%dimensions.width; }
        case 1u: { x=(x+dimensions.width-1u)%dimensions.width; }
        case 2u: { y=(y+1u)%dimensions.height; }
        case 3u: { y=(y+dimensions.height-1u)%dimensions.height; }
        case 4u: { z=(z+1u)%dimensions.depth; }
        default: { z=(z+dimensions.depth-1u)%dimensions.depth; }
    }
    return (z*dimensions.height+y)*dimensions.width+x;
}
fn sends(position:u32,direction:u32)->u32 {
    let energy=source[position];
    var budget=energy;
    for(var d=0u;d<6u;d+=1u) {
        let other=source[neighbor(position,d)];
        if(budget>0u && energy>other && energy-other>=2u) {
            budget-=1u;
            if(d==direction) { return 1u; }
        }
    }
    return 0u;
}
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) invocation:vec3<u32>,@builtin(num_workgroups) groups:vec3<u32>) {
    let position=invocation.x+invocation.y*groups.x*64u;
    if(position>=dimensions.count) { return; }
    var outgoing=0u;
    for(var d=0u;d<6u;d+=1u) { outgoing+=sends(position,d); }
    var result=source[position]-outgoing;
    for(var d=0u;d<6u;d+=1u) {
        let amount=sends(neighbor(position,d),d^1u);
        if(amount>0u && result==0xffffffffu) { atomicStore(&status,1u); return; }
        result+=amount;
    }
    output_heat[position]=result;
}
