use std::collections::HashMap;
use std::sync::Arc;

use crate::net::minecraft::client::network::NetHandlerPlayClient::{PlayClientState,PlayerPositionState};
use crate::net::minecraft::client::renderer::BlockModelShapes::{BlockModelShapes,ResolvedBlockModel};
use crate::net::minecraft::client::resources::SimpleReloadableResourceManager::ResourceManager;
use crate::net::minecraft::util::EnumFacing::EnumFacing;
use crate::net::minecraft::util::ResourceLocation::ResourceLocation;
use crate::net::minecraft::util::math::BlockPos::BlockPos;
use crate::vulkan::CpuFrame::CpuFrame;
use crate::vulkan::TextureSource::TextureSource;

const MAX_INTERNAL_WIDTH:u32=480;
const MIN_INTERNAL_WIDTH:u32=160;
const MAX_RAY_STEPS:usize=512;

#[derive(Debug,Clone,Copy,PartialEq)]
struct CameraKey{position:PlayerPositionState,worldRevision:u64,width:u32,height:u32,fovBits:u32,renderDistance:i32}
#[derive(Debug,Clone)]
struct FrameCache{key:CameraKey,frame:CpuFrame}

/// Transitional world rasterizer. It consumes real `WorldClient` chunk data,
/// MCP block-state identities, MCP model JSON and original texture resources,
/// then presents the resulting frame through the existing Vulkan upload path.
/// It is intentionally isolated from MCP classes and will be replaced by the
/// native chunk-mesh Vulkan renderer without changing network/world semantics.
pub struct SoftwareWorldRenderer{
    blockModelShapes:BlockModelShapes,
    resourceManager:ResourceManager,
    textures:HashMap<ResourceLocation,Arc<TextureSource>>,
    frameCache:Option<FrameCache>,
}
impl SoftwareWorldRenderer{
    pub fn new(resourceManager:ResourceManager)->Self{
        Self{blockModelShapes:BlockModelShapes::new(resourceManager.clone()),resourceManager,textures:HashMap::new(),frameCache:None}
    }
    pub fn clearCaches(&mut self){self.textures.clear();self.frameCache=None;}
    pub fn render(&mut self,state:&PlayClientState,outputWidth:u32,outputHeight:u32,fov:f32,renderDistanceChunks:i32)->anyhow::Result<CpuFrame>{
        let width=outputWidth.max(1);let height=outputHeight.max(1);
        let key=CameraKey{position:state.playerPosition,worldRevision:state.revision,width,height,fovBits:fov.to_bits(),renderDistance:renderDistanceChunks};
        if let Some(cache)=&self.frameCache{if cache.key==key{return Ok(cache.frame.clone());}}
        let Some(world)=state.worldClient.as_ref() else{
            let mut frame=CpuFrame::new(width,height);frame.clear([0,0,0,255]);return Ok(frame);
        };
        let internalWidth=width.min(MAX_INTERNAL_WIDTH).max(MIN_INTERNAL_WIDTH.min(width));
        let internalHeight=((internalWidth as f64*height as f64/width as f64).round() as u32).max(1);
        let sky=sky_color(world.getDimension());
        let mut low=CpuFrame::new(internalWidth,internalHeight);low.clear([sky[0],sky[1],sky[2],255]);
        let camera=[state.playerPosition.posX,state.playerPosition.posY+state.playerPosition.eyeHeight as f64,state.playerPosition.posZ];
        let basis=camera_basis(state.playerPosition.rotationYaw,state.playerPosition.rotationPitch);
        let aspect=internalWidth as f64/internalHeight as f64;
        let tangent=(fov.clamp(30.0,110.0) as f64*0.5).to_radians().tan();
        let maxDistance=(renderDistanceChunks.clamp(2,12)*16) as f64;
        for y in 0..internalHeight{
            let screenY=(1.0-2.0*((y as f64+0.5)/internalHeight as f64))*tangent;
            for x in 0..internalWidth{
                let screenX=(2.0*((x as f64+0.5)/internalWidth as f64)-1.0)*tangent*aspect;
                let ray=normalize(add3(basis.forward,add3(scale3(basis.right,screenX),scale3(basis.up,screenY))));
                let color=self.trace(world,camera,ray,maxDistance,sky);
                low.set_pixel(x,y,color);
            }
        }
        let frame=if internalWidth==width&&internalHeight==height{low}else{scale_nearest(&low,width,height)};
        self.frameCache=Some(FrameCache{key,frame:frame.clone()});
        Ok(frame)
    }
    fn trace(&mut self,world:&crate::net::minecraft::client::multiplayer::WorldClient::WorldClient,origin:[f64;3],direction:[f64;3],maxDistance:f64,sky:[u8;3])->[u8;4]{
        let mut voxel=[origin[0].floor() as i32,origin[1].floor() as i32,origin[2].floor() as i32];
        let step=[sign(direction[0]),sign(direction[1]),sign(direction[2])];
        let delta=[inv_abs(direction[0]),inv_abs(direction[1]),inv_abs(direction[2])];
        let mut side=[initial_side(origin[0],voxel[0],direction[0]),initial_side(origin[1],voxel[1],direction[1]),initial_side(origin[2],voxel[2],direction[2])];
        let mut distance=0.0;let mut entered=EnumFacing::North;
        let mut accumulated=[0.0_f32;4];
        for _ in 0..MAX_RAY_STEPS{
            let axis=if side[0]<side[1]&&side[0]<side[2]{0}else if side[1]<side[2]{1}else{2};
            distance=side[axis];if distance>maxDistance{break;}
            side[axis]+=delta[axis];voxel[axis]+=step[axis];
            entered=match (axis,step[axis]){(0,1)=>EnumFacing::West,(0,_)=>EnumFacing::East,(1,1)=>EnumFacing::Down,(1,_)=>EnumFacing::Up,(2,1)=>EnumFacing::North,_=>EnumFacing::South};
            if !(0..256).contains(&voxel[1]){continue;}
            let pos=BlockPos::new(voxel[0],voxel[1],voxel[2]);
            let state=world.getBlockState(pos);if state.isAir(){continue;}
            let Some(model)=self.blockModelShapes.getModelForState(state) else{continue;};
            if !model.fullCube{continue;}
            let hit=add3(origin,scale3(direction,distance+1.0e-7));
            let mut sample=self.sample_face(&model,entered,hit,state.getBlockId());
            if sample[3]<=0.01{continue;}
            let faceShade=match entered{EnumFacing::Down=>0.5,EnumFacing::Up=>1.0,EnumFacing::North|EnumFacing::South=>0.8,EnumFacing::West|EnumFacing::East=>0.6};
            let light=(world.getCombinedLightLevel(pos) as f32/15.0).max(0.18);
            for channel in 0..3{sample[channel]*=faceShade as f32*light;}
            let fog=(distance/maxDistance).clamp(0.0,1.0);let fog=fog*fog;
            for channel in 0..3{sample[channel]=sample[channel]*(1.0-fog as f32)+(sky[channel] as f32/255.0)*fog as f32;}
            let remaining=1.0-accumulated[3];
            for channel in 0..3{accumulated[channel]+=sample[channel]*sample[3]*remaining;}
            accumulated[3]+=sample[3]*remaining;
            if accumulated[3]>=0.98{break;}
        }
        if accumulated[3]<1.0{
            let remaining=1.0-accumulated[3];for channel in 0..3{accumulated[channel]+=(sky[channel] as f32/255.0)*remaining;}accumulated[3]=1.0;
        }
        [(accumulated[0].clamp(0.0,1.0)*255.0) as u8,(accumulated[1].clamp(0.0,1.0)*255.0) as u8,(accumulated[2].clamp(0.0,1.0)*255.0) as u8,255]
    }
    fn sample_face(&mut self,model:&ResolvedBlockModel,facing:EnumFacing,hit:[f64;3],blockId:i32)->[f32;4]{
        let face=model.face(facing);if face.layers.is_empty(){return [0.0,0.0,0.0,0.0];}
        let fraction=[fract(hit[0]),fract(hit[1]),fract(hit[2])];
        let (u,v)=match facing{
            EnumFacing::Down=>(fraction[0],1.0-fraction[2]),EnumFacing::Up=>(fraction[0],fraction[2]),
            EnumFacing::North=>(1.0-fraction[0],1.0-fraction[1]),EnumFacing::South=>(fraction[0],1.0-fraction[1]),
            EnumFacing::West=>(fraction[2],1.0-fraction[1]),EnumFacing::East=>(1.0-fraction[2],1.0-fraction[1]),
        };
        let mut output=[0.0_f32;4];
        for layer in &face.layers{
            let texture=self.texture(&layer.texture);
            let image=&texture.image;
            let x=((u.clamp(0.0,0.999999)*image.width() as f64) as u32).min(image.width()-1);
            let y=((v.clamp(0.0,0.999999)*image.height() as f64) as u32).min(image.height()-1);
            let pixel=image.pixel_rgba(x,y);let tint=tint_color(blockId,layer.tintIndex);
            let source=[pixel[0] as f32/255.0*tint[0],pixel[1] as f32/255.0*tint[1],pixel[2] as f32/255.0*tint[2],pixel[3] as f32/255.0];
            let remaining=1.0-output[3];for channel in 0..3{output[channel]+=source[channel]*source[3]*remaining;}output[3]+=source[3]*remaining;
        }
        if output[3]>0.0{for channel in 0..3{output[channel]/=output[3];}}
        output
    }
    fn texture(&mut self,location:&ResourceLocation)->Arc<TextureSource>{
        if let Some(texture)=self.textures.get(location){return Arc::clone(texture);}
        let texture=Arc::new(TextureSource::load(&self.resourceManager,location).unwrap_or_else(|error|{log::warn!("failed loading world texture {location}: {error}");TextureSource::missing(location.clone())}));
        self.textures.insert(location.clone(),Arc::clone(&texture));texture
    }
}
#[derive(Clone,Copy)]struct CameraBasis{forward:[f64;3],right:[f64;3],up:[f64;3]}
fn camera_basis(yaw:f32,pitch:f32)->CameraBasis{
    let yaw=(-yaw as f64).to_radians()-std::f64::consts::PI;let pitch=(-pitch as f64).to_radians();
    let forward=normalize([yaw.sin()*-pitch.cos(),pitch.sin(),yaw.cos()*-pitch.cos()]);
    let worldUp=[0.0,1.0,0.0];let right=normalize(cross(forward,worldUp));let up=normalize(cross(right,forward));CameraBasis{forward,right,up}
}
fn sky_color(dimension:i32)->[u8;3]{match dimension{-1=>[48,8,8],1=>[8,8,16],_=>[126,178,255]}}
fn tint_color(blockId:i32,tint:Option<i32>)->[f32;3]{if tint.is_none(){return [1.0;3];}let color=match blockId{2=>0x91BD59,18|161=>0x77AB2F,106=>0x48B518,_=>0xFFFFFF};[((color>>16)&255) as f32/255.0,((color>>8)&255) as f32/255.0,(color&255) as f32/255.0]}
fn scale_nearest(source:&CpuFrame,width:u32,height:u32)->CpuFrame{let mut out=CpuFrame::new(width,height);for y in 0..height{let sy=(y as u64*source.height() as u64/height as u64) as u32;for x in 0..width{let sx=(x as u64*source.width() as u64/width as u64) as u32;out.set_pixel(x,y,source.pixel(sx,sy));}}out}
fn sign(value:f64)->i32{if value<0.0{-1}else{1}}
fn inv_abs(value:f64)->f64{if value.abs()<1.0e-12{f64::INFINITY}else{1.0/value.abs()}}
fn initial_side(origin:f64,voxel:i32,direction:f64)->f64{if direction>0.0{(voxel as f64+1.0-origin)/direction}else if direction<0.0{(origin-voxel as f64)/-direction}else{f64::INFINITY}}
fn fract(value:f64)->f64{value-value.floor()}
fn add3(a:[f64;3],b:[f64;3])->[f64;3]{[a[0]+b[0],a[1]+b[1],a[2]+b[2]]}
fn scale3(a:[f64;3],s:f64)->[f64;3]{[a[0]*s,a[1]*s,a[2]*s]}
fn cross(a:[f64;3],b:[f64;3])->[f64;3]{[a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]]}
fn normalize(a:[f64;3])->[f64;3]{let length=(a[0]*a[0]+a[1]*a[1]+a[2]*a[2]).sqrt();if length<=1.0e-12{[0.0,0.0,1.0]}else{[a[0]/length,a[1]/length,a[2]/length]}}

#[cfg(test)]mod tests{use super::*;#[test]fn yaw_zero_faces_positive_z(){let basis=camera_basis(0.0,0.0);assert!(basis.forward[2]>0.999);assert!(basis.forward[0].abs()<1.0e-6);}#[test]fn nearest_scale_preserves_corners(){let mut source=CpuFrame::new(2,2);source.set_pixel(0,0,[1,2,3,4]);source.set_pixel(1,1,[5,6,7,8]);let out=scale_nearest(&source,4,4);assert_eq!(out.pixel(0,0),[1,2,3,4]);assert_eq!(out.pixel(3,3),[5,6,7,8]);}}
