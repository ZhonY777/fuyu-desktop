//! 浮屿应用管理与桌面分区。同一 egui 进程，不启动 DeskBox 或桥接进程。
use eframe::egui::{self,Color32,Vec2};
use serde::{Serialize,Deserialize};
use std::{path::{Path,PathBuf},time::{Duration,Instant},sync::atomic::{AtomicBool,Ordering}};
use crate::app::Cmd;

#[derive(Clone,Serialize,Deserialize,Default)]
#[serde(default)]
pub struct Entry {pub name:String,pub path:PathBuf,pub category:String,pub favorite:bool,pub hidden:bool,pub manual:bool}
#[derive(Clone,Serialize,Deserialize)]
#[serde(default)]
pub struct Folder {pub name:String,pub path:PathBuf,pub position:[f32;2],pub size:[f32;2],pub visible:bool,pub group:String}
impl Default for Folder {fn default()->Self{Self{name:"收纳".into(),path:PathBuf::new(),position:[520.,90.],size:[340.,520.],visible:true,group:"资料".into()}}}
#[derive(Serialize,Deserialize)]
#[serde(default)]
pub struct Config {pub apps:Vec<Entry>,pub folders:Vec<Folder>,pub opacity:f32,pub show_on_desktop:bool,pub hide_icons:Option<bool>,pub hotkey:u32,pub position:[f32;2],pub size:[f32;2],pub icon_size:f32,pub notes:String,pub active_folders:std::collections::HashMap<String,usize>,pub categories:Vec<String>}
impl Default for Config {fn default()->Self{Self{apps:vec![],folders:vec![],opacity:0.65,show_on_desktop:true,hide_icons:None,hotkey:118,position:[24.,100.],size:[460.,650.],icon_size:40.,notes:String::new(),active_folders:Default::default(),categories:vec![]}}}
impl Config {
    fn path()->PathBuf{crate::model::config_dir().join("desktop.json")}
    fn load()->Self{std::fs::read(Self::path()).ok().and_then(|v|serde_json::from_slice(&v).ok()).unwrap_or_default()}
    fn save(&self){if let Ok(v)=serde_json::to_vec_pretty(self){let _=std::fs::create_dir_all(crate::model::config_dir());let _=crate::model::atomic_write(&Self::path(),&v);}}
}
pub struct Hub {pub cfg:Config,pub open:bool,pub request:bool,search:String,category:String,icons:crate::icons::Cache,refresh:Instant,folder_entries:Vec<Vec<PathBuf>>,folder_path:String,folder_name:String,new_category:String,old_category:String,initialized:bool,pub message:String,auto_hidden:bool,last_visible:bool,settings_requested:bool,discovery:Instant,folder_group:String,renames:std::collections::HashMap<PathBuf,String>,pending_delete:Option<PathBuf>}
impl Default for Hub {fn default()->Self{Self{cfg:Config::load(),open:false,request:false,search:String::new(),category:"常用".into(),icons:crate::icons::Cache::with_capacity(256),refresh:Instant::now(),folder_entries:vec![],folder_path:String::new(),folder_name:String::new(),new_category:String::new(),old_category:String::new(),initialized:false,message:String::new(),auto_hidden:false,last_visible:false,settings_requested:false,discovery:Instant::now()+Duration::from_secs(60),folder_group:"资料".into(),renames:Default::default(),pending_delete:None}}}
static TOGGLE:AtomicBool=AtomicBool::new(false);

pub fn is_hosted() -> bool {
    #[cfg(all(windows, feature = "embedded"))]
    { crate::embedded::is_hosted() }
    #[cfg(not(all(windows, feature = "embedded")))]
    { false }
}

impl crate::app::App {
    pub fn open_desktop(&mut self){
        #[cfg(all(windows, feature = "embedded"))]
        if crate::embedded::request_desktop(false) {
            return;
        }
        self.desktop.request_open();
    }
    pub fn desktop_ui(&mut self, ctx: &egui::Context, hosted: bool) {
        self.pending.extend(self.desktop.ui_mode(ctx, hosted));
        if std::mem::take(&mut self.desktop.settings_requested) {
            self.settings_tab = 12;
            self.open_settings();
        }
    }
}
impl Drop for Hub {
    fn drop(&mut self){if !is_hosted()&&self.initialized&&self.cfg.hide_icons==Some(true)&&std::env::var_os("FUYU_PREVIEW").is_none(){native::hide_icons(false);}}
}
pub fn restore_desktop(){native::hide_icons(false);let mut cfg=Config::load();cfg.hide_icons=Some(false);cfg.save();}
pub fn exit_cleanup(){if std::env::var_os("FUYU_PREVIEW").is_none(){native::hide_icons(false);}}
pub fn self_check()->Result<(),String>{
    let cfg=Config::load();let settings=crate::model::Settings::load();
    if cfg.apps.is_empty(){return Err("未迁入应用入口".into());}
    let missing:Vec<_>=cfg.apps.iter().filter(|a|!a.path.exists()).map(|a|a.name.clone()).collect();
    let body=serde_json::json!({"process":std::process::id(),"apps":cfg.apps.len(),"folders":cfg.folders.len(),"missing_apps":missing,"all_original_pages":settings.visible_pages.iter().all(|p|*p),"architecture":"one-process; native Rust modules; no DeskBox/Daozai child process"});
    std::fs::write(crate::model::config_dir().join("unified-check.json"),serde_json::to_vec_pretty(&body).unwrap()).map_err(|e|e.to_string())?;Ok(())
}
pub fn classify(name:&str)->String{crate::classification::classify(name)}
fn supported(p:&Path)->bool{p.extension().and_then(|e|e.to_str()).is_some_and(|e|["lnk","url","exe","appref-ms"].contains(&e.to_lowercase().as_str()))}
fn scan(dir:&Path,recursive:bool,out:&mut Vec<PathBuf>){
    if let Ok(entries)=std::fs::read_dir(dir){for e in entries.flatten(){let p=e.path();if p.is_file()&&supported(&p){out.push(p)}else if recursive&&p.is_dir()&&!e.file_type().is_ok_and(|t|t.is_symlink()){scan(&p,true,out)}}}
}
fn desktop_paths()->Vec<(String,PathBuf)>{
    let home=std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default();let public=std::env::var_os("PUBLIC").map(PathBuf::from).unwrap_or_else(||PathBuf::from("C:/Users/Public"));
    vec![("桌面收纳".into(),home.join("Desktop")),("公共桌面".into(),public.join("Desktop"))]
}
fn copy_into(source:&Path,directory:&Path)->Result<PathBuf,String>{
    let src=source.canonicalize().map_err(|e|e.to_string())?;let dir=directory.canonicalize().map_err(|e|e.to_string())?;
    let name=source.file_name().ok_or("缺少文件名")?;let initial=dir.join(name);
    if initial.exists()&&initial.canonicalize().ok().is_some_and(|p|p.to_string_lossy().eq_ignore_ascii_case(&src.to_string_lossy())){return Ok(initial);}
    if src.is_dir()&&dir.to_string_lossy().to_lowercase().starts_with(&src.to_string_lossy().to_lowercase()){return Err("不能将文件夹复制到自身内部".into());}
    let mut target=initial;for n in 2..10000{if !target.exists(){break;}let stem=if source.is_file(){source.file_stem().unwrap_or(name)}else{name}.to_string_lossy();let ext=if source.is_file(){source.extension().map(|e|format!(".{}",e.to_string_lossy())).unwrap_or_default()}else{String::new()};target=dir.join(format!("{stem} ({n}){ext}"));}
    fn copy_tree(src:&Path,dst:&Path,depth:u32)->Result<(),String>{if depth>64{return Err("文件夹层级过深".into());}let meta=std::fs::symlink_metadata(src).map_err(|e|e.to_string())?;if meta.file_type().is_symlink(){return Err("请使用普通文件夹，或拖入 .lnk 快捷方式".into());}if meta.is_dir(){std::fs::create_dir(dst).map_err(|e|e.to_string())?;for e in std::fs::read_dir(src).map_err(|e|e.to_string())?{let e=e.map_err(|e|e.to_string())?;copy_tree(&e.path(),&dst.join(e.file_name()),depth+1)?;}}else{std::fs::copy(src,dst).map_err(|e|e.to_string())?;}Ok(())}
    copy_tree(&src,&target,0)?;Ok(target)
}
impl Hub {
    pub(crate) fn apply_hosted_icon_preference(&self) {
        if !in_preview() {
            if let Some(hidden) = self.cfg.hide_icons {
                native::hide_icons(hidden);
            }
        }
    }

    pub fn request_open(&mut self){self.open=true;self.request=true;self.auto_hidden=false;}
    fn refresh_files(&mut self){
        self.folder_entries=self.cfg.folders.iter().map(|f|{let mut v:Vec<_>=std::fs::read_dir(&f.path).into_iter().flatten().flatten().map(|e|e.path()).filter(|p|p.file_name().is_some_and(|n|n!="desktop.ini")).collect();v.sort_by_key(|p|(!p.is_dir(),p.file_name().unwrap_or_default().to_ascii_lowercase()));v}).collect();
        self.refresh=Instant::now()+Duration::from_millis(900);
    }
    pub fn discover(&mut self){
        let mut paths=vec![];
        if let Some(a)=std::env::var_os("APPDATA"){scan(&PathBuf::from(a).join("Microsoft/Windows/Start Menu/Programs"),true,&mut paths)}
        if let Some(a)=std::env::var_os("ProgramData"){scan(&PathBuf::from(a).join("Microsoft/Windows/Start Menu/Programs"),true,&mut paths)}
        for(_,p)in desktop_paths(){scan(&p,false,&mut paths)}
        let mut added=0;for p in paths {let name=p.file_stem().unwrap_or_default().to_string_lossy().into_owned();if name.contains("浮屿")||name.contains("岛仔")||name.eq_ignore_ascii_case("DeskBox"){continue;}if !self.cfg.apps.iter().any(|a|a.path==p||a.name.eq_ignore_ascii_case(&name)){self.cfg.apps.push(Entry{name:name.clone(),category:crate::classification::entry_category(&name,&p),path:p,..Default::default()});added+=1;}}
        self.cfg.save();self.message=format!("已补充 {added} 个入口；手动分类保持不变");
    }
    pub fn settings_ui(&mut self,ui:&mut egui::Ui){
        let hosted = is_hosted();
        ui.visuals_mut().extreme_bg_color=if ui.visuals().dark_mode{Color32::from_rgb(20,24,30)}else{Color32::from_rgb(245,247,250)};
        ui.visuals_mut().widgets.inactive.bg_stroke=egui::Stroke::new(1.0,Color32::from_gray(if ui.visuals().dark_mode{75}else{205}));
        ui.label("应用图标、桌面文件分区与顶部灵动岛在同一个程序内运行。");
        if ui.button("显示应用面板").clicked(){self.request_open()}
        if ui.button("重新扫描开始菜单与桌面").clicked(){self.discover()}
        let mut changed=false;
        changed|=ui.add(egui::Slider::new(&mut self.cfg.opacity,0.0..=1.0).text("面板与图标底色不透明度")).changed();
        changed|=ui.add(egui::Slider::new(&mut self.cfg.icon_size,24.0..=64.0).text("图标大小")).changed();
        if !hosted {
        changed|=ui.checkbox(&mut self.cfg.show_on_desktop,"回到桌面时显示文件分区").changed();
        }
        let mut hide = self.cfg.hide_icons.unwrap_or(false);
        if ui.checkbox(&mut hide, "隐藏 Windows 原桌面图标").changed() {
            self.cfg.hide_icons = Some(hide);
            if !in_preview() {
                native::hide_icons(hide);
            }
            changed = true;
        }
        if !hosted {
        ui.horizontal(|ui|{ui.label("面板快捷键");for(k,name)in[(118,"F7"),(119,"F8"),(120,"F9")]{if ui.selectable_value(&mut self.cfg.hotkey,k,name).changed(){changed=true;}}});
        ui.small("快捷键更改后重启生效；与其他软件冲突时会在面板中提示。");
        ui.separator();ui.label("映射文件夹（文件变化自动刷新）");
        ui.horizontal(|ui|{ui.label("名称");ui.text_edit_singleline(&mut self.folder_name);});
        ui.horizontal(|ui|{ui.label("路径");ui.text_edit_singleline(&mut self.folder_path);});
        ui.horizontal(|ui|{ui.label("分组");ui.text_edit_singleline(&mut self.folder_group);});
        if ui.button("添加桌面分区").clicked(){let path=PathBuf::from(self.folder_path.trim());if path.is_dir(){self.cfg.folders.push(Folder{name:if self.folder_name.trim().is_empty(){path.file_name().unwrap_or_default().to_string_lossy().into_owned()}else{self.folder_name.trim().into()},path,group:self.folder_group.trim().into(),..Default::default()});changed=true;self.refresh=Instant::now();}else{self.message="请选择存在的文件夹".into();}}
        let mut remove=None;for(i,f)in self.cfg.folders.iter_mut().enumerate(){ui.horizontal(|ui|{changed|=ui.checkbox(&mut f.visible,&f.name).changed();ui.add_sized([(ui.available_width()-82.).max(40.),20.],egui::Label::new(egui::RichText::new(f.path.display().to_string()).small()).truncate()).on_hover_text(f.path.display().to_string());if ui.small_button("移除映射").clicked(){remove=Some(i)}});}
        if let Some(i)=remove{self.cfg.folders.remove(i);changed=true;self.refresh=Instant::now();}
        }
        ui.separator();ui.label("分类管理");
        ui.horizontal(|ui|{ui.label("原分类");ui.text_edit_singleline(&mut self.old_category);});
        ui.horizontal(|ui|{ui.label("新分类");ui.text_edit_singleline(&mut self.new_category);});
        if ui.button("新建分类").clicked()&&!self.new_category.trim().is_empty(){let c=self.new_category.trim().to_owned();if !self.cfg.categories.contains(&c){self.cfg.categories.push(c);}changed=true;}
        if ui.button("重命名分类").clicked()&&!self.new_category.trim().is_empty(){for a in &mut self.cfg.apps{if a.category==self.old_category.trim(){a.category=self.new_category.trim().into();a.manual=true;}}for c in &mut self.cfg.categories{if c==self.old_category.trim(){*c=self.new_category.trim().into();}}changed=true;}
        if ui.button("恢复隐藏的应用入口").clicked(){for a in &mut self.cfg.apps{a.hidden=false;}changed=true;}
        ui.separator();ui.label("随手记");changed|=ui.add(egui::TextEdit::multiline(&mut self.cfg.notes).desired_rows(6).desired_width(f32::INFINITY)).changed();
        if !self.message.is_empty(){ui.label(&self.message);}
        if changed{self.cfg.save();}
    }
    #[cfg(test)]
    pub fn ui(&mut self,ctx:&egui::Context)->Vec<Cmd>{
        self.ui_mode(ctx, false)
    }

    fn ui_mode(&mut self,ctx:&egui::Context,hosted:bool)->Vec<Cmd>{
        let mut cmds=vec![];
        if hosted && !self.open {
            return cmds;
        }
        if !self.initialized {
            self.initialized=true;
            if self.cfg.apps.is_empty(){self.discover()}
            if !hosted {
                if self.cfg.folders.is_empty(){self.cfg.folders=desktop_paths().into_iter().filter(|(_,p)|p.is_dir()).map(|(name,path)|Folder{name,path,..Default::default()}).collect();self.cfg.save();}
                if std::env::var_os("FUYU_PREVIEW").is_none(){if let Some(hide)=self.cfg.hide_icons{native::hide_icons(hide);}native::hotkey(self.cfg.hotkey,ctx.clone());}
            }
        }
        if TOGGLE.swap(false,Ordering::Relaxed){if self.last_visible{self.open=false;self.auto_hidden=true;}else{self.request_open();}}
        if !hosted && Instant::now()>=self.refresh{self.refresh_files();}
        if Instant::now()>=self.discovery{self.discover();self.discovery=Instant::now()+Duration::from_secs(60);}
        let desktop=native::on_desktop();let show=self.open||(self.cfg.show_on_desktop&&!self.auto_hidden&&(desktop||native::on_panel()));self.last_visible=show;
        let opacity=(self.cfg.opacity*255.0).round()as u8;
        if show {
            let focus=std::mem::take(&mut self.request);let mut close=false;let mut save=false;
            let builder=egui::ViewportBuilder::default().with_title("浮屿 · 应用分类").with_inner_size(self.cfg.size).with_position(self.cfg.position).with_min_inner_size([340.,280.]).with_transparent(true).with_decorations(false).with_taskbar(std::env::var_os("FUYU_PREVIEW").is_some()).with_active(false).with_visible(false);
            ctx.show_viewport_immediate(egui::ViewportId::from_hash_of("fuyu-apps"),builder,|ctx,_|{
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                if focus{ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));ctx.send_viewport_cmd(egui::ViewportCommand::Focus);}
                if desktop{ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::AlwaysOnTop));}else{ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::Normal));}
                if ctx.input(|i|i.viewport().close_requested()){close=true;}
                egui::CentralPanel::default().frame(egui::Frame::new().fill(Color32::from_rgba_unmultiplied(19,29,41,opacity)).corner_radius(14.0).inner_margin(12)).show(ctx,|ui|{
                    header(ui,"浮屿 · 应用分类",&mut close);
                    if ui.small_button("统一设置").clicked(){self.settings_requested=true;}
                    ui.horizontal(|ui|{let r=ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("搜索应用 · Ctrl+F").desired_width(ui.available_width()-60.));if focus||ui.input(|i|i.modifiers.ctrl&&i.key_pressed(egui::Key::F)){r.request_focus();}if ui.button("清除").clicked(){self.search.clear();}});
                    let mut cats=vec!["常用".to_owned(),"全部".to_owned()];for a in &self.cfg.apps{if !a.hidden&&!cats.contains(&a.category){cats.push(a.category.clone())}}for c in &self.cfg.categories{if !cats.contains(c){cats.push(c.clone())}}
                    ui.horizontal_wrapped(|ui|{for c in &cats{ui.selectable_value(&mut self.category,c.clone(),c);}});ui.separator();
                    let mut entries:Vec<(usize,Entry)>=self.cfg.apps.iter().enumerate().filter(|(_,a)|!a.hidden&&(!self.search.is_empty()||self.category=="全部"||(self.category=="常用"&&a.favorite)||a.category==self.category)&&a.name.to_lowercase().contains(&self.search.to_lowercase())).map(|(i,a)|(i,a.clone())).collect();entries.sort_by_key(|(_,a)|(!a.favorite,a.name.to_lowercase()));
                    if ui.input(|i|i.key_pressed(egui::Key::Enter)){if let Some((_,a))=entries.first(){cmds.push(Cmd::Open(a.path.clone()));}}
                    if ui.input(|i|i.key_pressed(egui::Key::Escape)){if self.search.is_empty(){close=true;}else{self.search.clear();}}
                    let cols=((ui.available_width()+8.)/88.).floor().max(1.)as usize;
                    egui::ScrollArea::vertical().auto_shrink([false,false]).show_rows(ui,self.cfg.icon_size+52.,entries.len().div_ceil(cols),|ui,rows|{for row_index in rows{let row=&entries[row_index*cols..entries.len().min((row_index+1)*cols)];ui.horizontal(|ui|{for(index,a)in row{
                        let response=tile(ui,&mut self.icons,&a.path,&a.name,self.cfg.icon_size,opacity);
                        if response.clicked(){cmds.push(Cmd::Open(a.path.clone()));}
                        response.context_menu(|ui|{
                            if ui.button(if a.favorite{"取消收藏"}else{"加入常用"}).clicked(){self.cfg.apps[*index].favorite=!a.favorite;save=true;ui.close_menu();}
                            ui.menu_button("移动到分类", |ui| {
                                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                                    for category in cats.iter().skip(2) {
                                        if ui.button(category).clicked() {
                                            self.cfg.apps[*index].category = category.clone();
                                            self.cfg.apps[*index].manual = true;
                                            save = true;
                                            ui.close_menu();
                                        }
                                    }
                                });
                            });
                            if ui.button("定位快捷方式").clicked(){cmds.push(Cmd::Reveal(a.path.clone()));ui.close_menu();}
                            if ui.button("隐藏入口").clicked(){self.cfg.apps[*index].hidden=true;save=true;ui.close_menu();}
                        });
                    }});}});
                    for f in ctx.input(|i|i.raw.dropped_files.clone()){if let Some(path)=f.path{if supported(&path)&&!self.cfg.apps.iter().any(|a|a.path==path){let name=path.file_stem().unwrap_or_default().to_string_lossy().into_owned();self.cfg.apps.push(Entry{name:name.clone(),path,category:if["常用","全部"].contains(&self.category.as_str()){classify(&name)}else{self.category.clone()},favorite:self.category=="常用",manual:true,hidden:false});save=true;}}}
                    ui.small("拖入软件或快捷方式添加；右键可收藏、分类或隐藏。");
                });
                if let Some(rect)=ctx.input(|i|i.viewport().outer_rect){let p=[rect.min.x,rect.min.y];let s=[rect.width(),rect.height()];if self.cfg.position!=p||self.cfg.size!=s{self.cfg.position=p;self.cfg.size=s;save=true;}}
            });
            if close{self.open=false;self.auto_hidden=true;}
            if save{self.cfg.save();}
        }
        let folders=if hosted { Vec::new() } else { self.cfg.folders.clone() };
        let mut groups:Vec<(String,Vec<usize>)>=vec![];
        for(i,f)in folders.iter().enumerate().filter(|(_,f)|f.visible){let key=if f.group.is_empty(){format!("folder-{i}")}else{f.group.clone()};if let Some((_,indices))=groups.iter_mut().find(|(k,_)|k==&key){indices.push(i);}else{groups.push((key,vec![i]));}}
        for(group,indices)in groups {if !show{continue;}
            let index=self.cfg.active_folders.get(&group).copied().filter(|i|indices.contains(i)).unwrap_or(indices[0]);let folder=&folders[index];
            let files=self.folder_entries.get(index).cloned().unwrap_or_default();let mut close=false;let mut pos=None;
            ctx.show_viewport_immediate(egui::ViewportId::from_hash_of(("fuyu-folder",&group)),egui::ViewportBuilder::default().with_title(format!("浮屿 · {}",group)).with_position(folder.position).with_inner_size(folder.size).with_min_inner_size([240.,200.]).with_transparent(true).with_decorations(false).with_taskbar(std::env::var_os("FUYU_PREVIEW").is_some()).with_active(false).with_visible(false),|ctx,_|{
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(if desktop{egui::WindowLevel::AlwaysOnTop}else{egui::WindowLevel::Normal}));
                egui::CentralPanel::default().frame(egui::Frame::new().fill(Color32::from_rgba_unmultiplied(19,29,41,opacity)).corner_radius(14.0).inner_margin(12)).show(ctx,|ui|{
                    header(ui,&group,&mut close);
                    ui.horizontal_wrapped(|ui|{for &i in &indices{if ui.selectable_label(i==index,&folders[i].name).clicked(){self.cfg.active_folders.insert(group.clone(),i);self.cfg.save();}}});
                    ui.horizontal(|ui|{if ui.small_button("打开目录").clicked(){cmds.push(Cmd::Open(folder.path.clone()));}if ui.small_button("新建文件夹").clicked(){for n in 1..1000{let p=folder.path.join(if n==1{"新建文件夹".into()}else{format!("新建文件夹 ({n})")});if !p.exists(){let _=std::fs::create_dir(p);self.refresh=Instant::now();break;}}}ui.small(format!("{} 项",files.len()));});
                    let cols=((ui.available_width()+8.)/88.).floor().max(1.)as usize;
                    egui::ScrollArea::vertical().auto_shrink([false,false]).show_rows(ui,self.cfg.icon_size+52.,files.len().div_ceil(cols),|ui,rows|{for row_index in rows{let row=&files[row_index*cols..files.len().min((row_index+1)*cols)];ui.horizontal(|ui|{for path in row{
                        let name=path.file_name().unwrap_or_default().to_string_lossy();let r=tile(ui,&mut self.icons,path,&name,self.cfg.icon_size,opacity);
                        if r.double_clicked(){cmds.push(Cmd::Open(path.clone()));}
                        r.context_menu(|ui|{
                            if ui.button("打开").clicked(){cmds.push(Cmd::Open(path.clone()));ui.close_menu();}
                            if ui.button("在资源管理器中显示").clicked(){cmds.push(Cmd::Reveal(path.clone()));ui.close_menu();}
                            if ui.button("复制路径").clicked(){cmds.push(Cmd::Copy(path.display().to_string()));ui.close_menu();}
                            if ui.button("拖出到其他应用").clicked(){cmds.push(Cmd::DragFile(path.clone()));ui.close_menu();}
                            ui.separator();let edit=self.renames.entry(path.clone()).or_insert_with(||name.to_string());ui.text_edit_singleline(edit);
                            if ui.button("重命名").clicked(){let new=edit.trim().to_owned();if new.is_empty()||new=="."||new==".."||new.chars().any(|c|"<>:\"/\\|?*".contains(c)){self.message="文件名包含不允许的字符".into();}else{let target=path.with_file_name(new);if target.exists(){self.message="同名文件已存在，未覆盖".into();}else{self.message=match std::fs::rename(path,target){Ok(())=>"已重命名".into(),Err(e)=>e.to_string()};self.refresh=Instant::now();}}ui.close_menu();}
                            if ui.button("移到回收站…").clicked(){self.pending_delete=Some(path.clone());ui.close_menu();}
                        });
                        if r.drag_started(){cmds.push(Cmd::DragFile(path.clone()));}
                    }});}});
                });
                for file in ctx.input(|i|i.raw.dropped_files.clone()){if let Some(source)=file.path{self.message=match copy_into(&source,&folder.path){Ok(_)=>"已复制到分区目录，原文件保留".into(),Err(e)=>e};self.refresh=Instant::now();}}
                if ctx.input(|i|i.viewport().close_requested()){close=true;}
                pos=ctx.input(|i|i.viewport().outer_rect).map(|r|([r.min.x,r.min.y],[r.width(),r.height()]));
            });
            if close{for &i in &indices{self.cfg.folders[i].visible=false;}self.cfg.save();}
            if let Some((p,s))=pos{if self.cfg.folders[index].position!=p||self.cfg.folders[index].size!=s{for &i in &indices{self.cfg.folders[i].position=p;self.cfg.folders[i].size=s;}self.cfg.save();}}
        }
        if let Some(path)=self.pending_delete.clone(){ctx.show_viewport_immediate(egui::ViewportId::from_hash_of("fuyu-delete-confirm"),egui::ViewportBuilder::default().with_title("浮屿 · 删除确认").with_inner_size([380.,160.]).with_always_on_top().with_taskbar(false),|ctx,_|{egui::CentralPanel::default().show(ctx,|ui|{ui.heading("移到回收站？");ui.label(path.file_name().unwrap_or_default().to_string_lossy());ui.label("这会操作真实文件，可从 Windows 回收站恢复。");ui.horizontal(|ui|{if ui.button("取消").clicked(){self.pending_delete=None;}if ui.button("确认移到回收站").clicked(){self.message=match native::recycle(&path){Ok(())=>"已移到回收站".into(),Err(e)=>e};self.pending_delete=None;self.refresh=Instant::now();}});});if ctx.input(|i|i.viewport().close_requested()){self.pending_delete=None;}});}
        ctx.request_repaint_after(Duration::from_millis(400));cmds
    }
}
fn header(ui:&mut egui::Ui,name:&str,close:&mut bool){
    ui.horizontal(|ui|{let r=ui.add_sized([(ui.available_width()-66.).max(80.),26.],egui::Label::new(egui::RichText::new(name).strong()).sense(egui::Sense::drag()));if r.drag_started(){ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);}let r=ui.add_sized([18.,24.],egui::Label::new("◢").sense(egui::Sense::drag()));if r.drag_started(){ui.ctx().send_viewport_cmd(egui::ViewportCommand::BeginResize(egui::ResizeDirection::SouthEast));}if ui.small_button("×").clicked(){*close=true;}});
}

fn in_preview() -> bool {
    std::env::var_os("FUYU_PREVIEW").is_some()
        || std::env::var_os("FUYU_PREVIEW_DATA_ROOT").is_some()
}
fn tile(ui:&mut egui::Ui,icons:&mut crate::icons::Cache,path:&Path,name:&str,size:f32,opacity:u8)->egui::Response{
    let r=egui::Frame::new().fill(Color32::from_rgba_unmultiplied(44,62,78,opacity/3)).corner_radius(9.0).show(ui,|ui|{
        ui.allocate_ui_with_layout(Vec2::new(78.,size+43.),egui::Layout::top_down(egui::Align::Center),|ui|{
            if let Some(p)=icons.get(ui.ctx(),path).and_then(|p|p.image){ui.add(egui::Image::new((p.id(),Vec2::splat(size))));}else{ui.add_sized([size,size],egui::Label::new("◇"));}
            let mut label=egui::text::LayoutJob::simple(name.to_owned(),egui::FontId::proportional(11.),ui.visuals().text_color(),74.);label.wrap.max_rows=2;label.wrap.break_anywhere=true;label.wrap.overflow_character=Some('…');ui.label(label);
        });
    }).response.interact(egui::Sense::click_and_drag());r.on_hover_text(path.display().to_string())
}

mod native {
    pub fn recycle(path:&std::path::Path)->Result<(),String>{
        #[cfg(windows)]unsafe{use std::os::windows::ffi::OsStrExt;use windows::{core::PCWSTR,Win32::UI::Shell::{SHFileOperationW,SHFILEOPSTRUCTW,FO_DELETE,FOF_ALLOWUNDO,FOF_NOCONFIRMATION}};let wide:Vec<u16>=path.as_os_str().encode_wide().chain([0,0]).collect();let mut op=SHFILEOPSTRUCTW{wFunc:FO_DELETE,pFrom:PCWSTR(wide.as_ptr()),fFlags:(FOF_ALLOWUNDO.0|FOF_NOCONFIRMATION.0)as u16,..Default::default()};let code=SHFileOperationW(&mut op);if code==0&&!op.fAnyOperationsAborted.as_bool(){Ok(())}else{Err(format!("文件操作未完成（{code}）"))}}
        #[cfg(not(windows))]{let _=path;Err("回收站仅支持 Windows".into())}
    }
    #[cfg(windows)]
    #[link(name="user32")]
    extern "system"{fn GetForegroundWindow()->isize;fn GetClassNameW(h:isize,s:*mut u16,n:i32)->i32;fn GetWindowTextW(h:*mut std::ffi::c_void,s:*mut u16,n:i32)->i32;fn FindWindowW(c:*const u16,n:*const u16)->isize;fn FindWindowExW(p:isize,a:isize,c:*const u16,n:*const u16)->isize;fn ShowWindow(h:isize,n:i32)->i32;fn EnumWindows(cb:unsafe extern "system" fn(*mut std::ffi::c_void,isize)->i32,p:isize)->i32;fn RegisterHotKey(h:isize,id:i32,mods:u32,key:u32)->i32;fn GetMessageW(m:*mut Msg,h:isize,min:u32,max:u32)->i32;}
    #[cfg(windows)] #[repr(C)]struct Msg{h:isize,id:u32,w:usize,l:isize,time:u32,x:i32,y:i32,private:u32}
    #[cfg(windows)] fn wide(s:&str)->Vec<u16>{s.encode_utf16().chain(Some(0)).collect()}
    pub fn on_desktop()->bool{
        #[cfg(windows)]unsafe{let mut s=[0u16;128];let n=GetClassNameW(GetForegroundWindow(),s.as_mut_ptr(),128);return ["Progman","WorkerW","SHELLDLL_DefView"].contains(&String::from_utf16_lossy(&s[..n.max(0)as usize]).as_str());}
        #[cfg(not(windows))]false
    }
    pub fn on_panel()->bool{
        #[cfg(windows)]unsafe{let mut s=[0u16;256];let n=GetWindowTextW(GetForegroundWindow() as *mut std::ffi::c_void,s.as_mut_ptr(),256);return String::from_utf16_lossy(&s[..n.max(0)as usize]).starts_with("浮屿 · ");}
        #[cfg(not(windows))]false
    }
    pub fn hotkey(key:u32,ctx:eframe::egui::Context){
        #[cfg(windows)]std::thread::spawn(move||unsafe{if RegisterHotKey(0,0x4655,0x4000,key)==0{return;}let mut msg:Msg=std::mem::zeroed();while GetMessageW(&mut msg,0,0,0)>0{if msg.id==0x312{super::TOGGLE.store(true,std::sync::atomic::Ordering::Relaxed);ctx.request_repaint();}}});
        #[cfg(not(windows))]let _=(key,ctx);
    }
    pub fn hide_icons(hide:bool){
        #[cfg(windows)]unsafe{
            use windows::{core::w,Win32::System::Registry::*};let mut key=HKEY::default();
            if RegCreateKeyExW(HKEY_CURRENT_USER,w!("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced"),0,None,REG_OPTION_NON_VOLATILE,KEY_SET_VALUE,None,&mut key,None).is_ok(){let value:u32=hide as u32;let _=RegSetValueExW(key,w!("HideIcons"),0,REG_DWORD,Some(&value.to_le_bytes()));let _=RegCloseKey(key);}
            let cls=wide("SHELLDLL_DefView");let mut view=FindWindowExW(FindWindowW(wide("Progman").as_ptr(),std::ptr::null()),0,cls.as_ptr(),std::ptr::null());
            unsafe extern "system" fn visit(h:*mut std::ffi::c_void,p:isize)->i32{let view=FindWindowExW(h as isize,0,wide("SHELLDLL_DefView").as_ptr(),std::ptr::null());if view!=0{*(p as *mut isize)=view;0}else{1}}
            if view==0{EnumWindows(visit,&mut view as *mut isize as isize);}
            if view!=0{let list=FindWindowExW(view,0,wide("SysListView32").as_ptr(),std::ptr::null());if list!=0{ShowWindow(list,if hide{0}else{5});}}
        }
        #[cfg(not(windows))]let _=hide;
    }
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn specific_categories(){assert_eq!(classify("QQ音乐"),"音乐与音频");assert_eq!(classify("QQ游戏"),"游戏与平台");assert_eq!(classify("QQ"),"聊天与会议");assert_eq!(classify("Visual Studio Code"),"编程与IDE");}
    #[test]fn state_roundtrip_keeps_manual_choices(){let mut c=Config::default();c.apps.push(Entry{name:"测试".into(),path:PathBuf::from("C:/Apps/test.lnk"),category:"自定义".into(),favorite:true,manual:true,..Default::default()});let b:Config=serde_json::from_slice(&serde_json::to_vec(&c).unwrap()).unwrap();assert!(b.apps[0].manual&&b.apps[0].favorite);assert_eq!(b.apps[0].category,"自定义");}
    #[test]fn copying_never_overwrites_an_existing_file(){let root=std::env::temp_dir().join(format!("fuyu-copy-check-{}",std::process::id()));std::fs::create_dir_all(root.join("destination")).unwrap();let source=root.join("note.txt");std::fs::write(&source,"new").unwrap();std::fs::write(root.join("destination/note.txt"),"keep").unwrap();let copied=copy_into(&source,&root.join("destination")).unwrap();assert_eq!(std::fs::read_to_string(root.join("destination/note.txt")).unwrap(),"keep");assert_eq!(std::fs::read_to_string(&copied).unwrap(),"new");assert_eq!(std::fs::read_to_string(&source).unwrap(),"new");assert!(copy_into(&root,&root.join("destination")).is_err());std::fs::remove_file(copied).unwrap();std::fs::remove_file(root.join("destination/note.txt")).unwrap();std::fs::remove_file(source).unwrap();std::fs::remove_dir(root.join("destination")).unwrap();std::fs::remove_dir(root).unwrap();}
    #[test]fn mapped_folder_tracks_create_rename_and_remove(){let root=std::env::temp_dir().join(format!("fuyu-inbox-check-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));std::fs::create_dir(&root).unwrap();let mut h=Hub::default();h.cfg.folders=vec![Folder{path:root.clone(),..Default::default()}];h.refresh_files();assert!(h.folder_entries[0].is_empty());let a=root.join("new.txt");let b=root.join("renamed.txt");std::fs::write(&a,"test").unwrap();h.refresh_files();assert_eq!(h.folder_entries[0],vec![a.clone()]);std::fs::rename(a,&b).unwrap();h.refresh_files();assert_eq!(h.folder_entries[0],vec![b.clone()]);std::fs::remove_file(b).unwrap();h.refresh_files();assert!(h.folder_entries[0].is_empty());std::fs::remove_dir(root).unwrap();}
    #[test]fn render_native_catalog_and_shared_settings(){
        let output=std::env::var_os("FUYU_QA_DIR").map(PathBuf::from);let Some(output)=output else{return};std::fs::create_dir_all(&output).unwrap();
        let ctx=egui::Context::default();ctx.set_embed_viewports(true);crate::theme::load_fonts(&ctx);ctx.set_visuals(egui::Visuals::dark());
        let mut hub=Hub::default();hub.initialized=true;hub.open=true;hub.cfg=Config::default();hub.cfg.show_on_desktop=false;
        for(name,cat)in[("Word","办公"),("Excel","办公"),("微信","通讯"),("Chatbox","AI"),("Visual Studio Code","开发"),("Steam","游戏")]{hub.cfg.apps.push(Entry{name:name.into(),path:PathBuf::from("C:/Windows/notepad.exe"),category:cat.into(),favorite:true,..Default::default()});}
        let mut raster=crate::preview_raster::Raster::new(720,650);
        for mode in ["catalog","settings"]{for frame in 0..8{
            let out=ctx.run(egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,Vec2::new(720.,650.))),time:Some(frame as f64/30.),..Default::default()},|ctx|{if mode=="catalog"{hub.ui(ctx);}else{egui::CentralPanel::default().show(ctx,|ui|{hub.settings_ui(ui);});}});
            raster.update_textures(&out.textures_delta);raster.buf.fill([0.05,0.07,0.1,1.]);let prims=ctx.tessellate(out.shapes,out.pixels_per_point);raster.draw(&prims,out.pixels_per_point);
        }let mut rgba=vec![];raster.to_rgba8(&mut rgba);image::save_buffer(output.join(format!("{mode}.png")),&rgba,720,650,image::ColorType::Rgba8).unwrap();}
    }
}





