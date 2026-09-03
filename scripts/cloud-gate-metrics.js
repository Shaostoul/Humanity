const sharp=require("sharp");const d=process.argv[2];
async function load(f){const {data,info}=await sharp(f).greyscale().raw().toBuffer({resolveWithObject:true});return {data,W:info.width,H:info.height};}
function spoke(img,CX,CY,R1){const {data,W}=img;const NA=720,R0=30;const A=new Float64Array(NA);for(let ai=0;ai<NA;ai++){const th=ai*2*Math.PI/NA,cs=Math.cos(th),sn=Math.sin(th);let s=0,n=0;for(let r=R0;r<R1;r+=0.5){const x=Math.round(CX+cs*r),y=Math.round(CY+sn*r);s+=data[y*W+x];n++;}A[ai]=s/n;}const half=48;let s=0;for(let i=0;i<NA;i++){let m=0;for(let k=-half;k<=half;k++)m+=A[(i+k+NA)%NA];m/=(2*half+1);s+=(A[i]-m)**2;}return Math.sqrt(s/NA);}
function grain(img){const {data,W,H}=img;let s=0,n=0;for(let y=Math.floor(H*0.2);y<H*0.8;y++)for(let x=Math.floor(W*0.2);x<W*0.8;x++){const i=y*W+x;s+=Math.abs(4*data[i]-data[i-1]-data[i+1]-data[i-W]-data[i+W]);n++;}return s/n;}
function frac(img){const {data,W,H}=img;let s=0,n=0,c=0;for(let y=Math.floor(H*0.2);y<H*0.8;y+=2)for(let x=Math.floor(W*0.2);x<W*0.8;x+=2){const l=data[y*W+x];s+=l;n++;if(l>235)c++;}return (s/n).toFixed(1)+" "+(100*c/n).toFixed(1)+"%";}
(async()=>{
 for(const [t,CY] of [["92",873],["dn",758]]){
  console.log("== "+t+" (spoke centred on nadir) ==");
  for(const dg of ["0","1","2","6"]) for(const c of ["b","e","ew"]){
   const f=d+"/tmp-"+t+"-d"+dg+"-"+c+".png"; if(!require("fs").existsSync(f)) continue;
   const img=await load(f);
   console.log("  diag"+dg+" "+c.padEnd(3)+" spoke "+spoke(img,1280,CY,300).toFixed(2).padEnd(7)+" grain "+grain(img).toFixed(2).padEnd(6)+" mean/cloud% "+frac(img));
  }
 }
 for(const t of ["92","dn"]){
  const c=[];for(const k of ["b","e"]) c.push(await sharp(d+"/tmp-"+t+"-d6-"+k+".png").extract({left:640,top:200,width:1280,height:1000}).resize(620,484).toBuffer());
  await sharp({create:{width:1256,height:484,channels:3,background:{r:20,g:20,b:20}}}).composite([{input:c[0],left:0,top:0},{input:c[1],left:636,top:0}]).png().toFile("scratch-depth-"+t+".png");
 }
 console.log("composites: scratch-depth-92.png, scratch-depth-dn.png (LEFT old march, RIGHT sample-anchored)");
})();
