//! 保留桌面待办的备注、子任务、附件、重要标记及完整重复周期。
use serde::{Serialize,Deserialize};
use chrono::{NaiveDate,NaiveDateTime,Datelike,Duration};
#[derive(Clone,Debug,Default,Serialize,Deserialize,PartialEq)]#[serde(default)]
pub struct Step {pub text:String,pub done:bool}
#[derive(Clone,Debug,Default,Serialize,Deserialize,PartialEq)]#[serde(default)]
pub struct Extra {pub notes:String,pub important:bool,pub steps:Vec<Step>,pub attachments:Vec<std::path::PathBuf>,pub recurrence:String,pub interval:u32,pub reminder_offset:i64,pub snoozed_until:Option<String>,pub last_reminded_due:Option<String>,pub legacy:serde_json::Value}
impl Extra {
    pub fn recurring(&self)->bool{["daily","weekly","monthly","yearly"].contains(&self.recurrence.as_str())}
    pub fn scheduled(&self,day:&str,due:Option<&str>,week:u8)->bool{
        let Ok(day)=NaiveDate::parse_from_str(day,"%Y-%m-%d")else{return false};
        let Some(start)=due.and_then(|s|s.get(..10)).and_then(|s|NaiveDate::parse_from_str(s,"%Y-%m-%d").ok())else{return self.recurrence=="daily"};
        if day<start{return false;}let every=self.interval.max(1)as i64;
        let last_day=|year:i32,month:u32|{let next=if month==12{NaiveDate::from_ymd_opt(year+1,1,1)}else{NaiveDate::from_ymd_opt(year,month+1,1)};next.map(|n|(n-Duration::days(1)).day()).unwrap_or(28)};
        match self.recurrence.as_str(){
            "daily"=>(day-start).num_days()%every==0,
            "weekly"=>{let anchor=start-Duration::days(start.weekday().num_days_from_monday()as i64);(day-anchor).num_days()/7%every==0&&(week==0||week&(1<<day.weekday().num_days_from_monday())!=0)},
            "monthly"=>{let months=(day.year()-start.year())as i64*12+day.month()as i64-start.month()as i64;months%every==0&&day.day()==start.day().min(last_day(day.year(),day.month()))},
            "yearly"=>(day.year()-start.year())as i64%every==0&&day.month()==start.month()&&day.day()==start.day().min(last_day(day.year(),day.month())),
            _=>true,
        }
    }
}
impl crate::model::TodoItem {
    fn latest_extra_occurrence(&self,now:&str)->Option<NaiveDateTime>{
        let now=NaiveDateTime::parse_from_str(now,"%Y-%m-%d %H:%M").ok()?;let due=NaiveDateTime::parse_from_str(self.due.as_deref()?,"%Y-%m-%d %H:%M").ok()?;
        if !self.is_repeating(){return Some(due);}
        let mut latest=None;let window=(self.extra.reminder_offset.max(0)/1440+2).min(365);
        for offset in -2..=window {let date=now.date().checked_add_signed(Duration::days(offset))?;let day=date.format("%Y-%m-%d").to_string();if self.scheduled_on(&day){let at=date.and_time(due.time());if at-Duration::minutes(self.extra.reminder_offset.clamp(0,525600))<=now{latest=Some(latest.map_or(at,|old:NaiveDateTime|old.max(at)));}}}latest
    }
    pub fn extra_due(&self,now:&str)->bool{
        if let Some(snooze)=self.extra.snoozed_until.as_deref(){return !self.done_for(now.get(..10).unwrap_or(""))&&snooze<=now;}
        let Some(at)=self.latest_extra_occurrence(now)else{return false};let Some(now_date)=now.get(..10)else{return false};
        if self.done_for(now_date){return false;}
        let reminder=at-Duration::minutes(self.extra.reminder_offset.clamp(0,525600));
        let Ok(now)=NaiveDateTime::parse_from_str(now,"%Y-%m-%d %H:%M")else{return false};
        let key=at.format("%Y-%m-%d %H:%M").to_string();reminder<=now&&if self.is_repeating(){self.extra.last_reminded_due.as_deref()!=Some(&key)}else{!self.reminded}
    }
    pub fn mark_extra(&mut self,now:&str){if let Some(at)=self.latest_extra_occurrence(now){self.extra.last_reminded_due=Some(at.format("%Y-%m-%d %H:%M").to_string());}self.extra.snoozed_until=None;}
}

impl crate::app::App {
    pub fn task_details_ui(&mut self,ctx:&eframe::egui::Context){
        let Some(index)=self.task_details else{return};if index>=self.todos.items.len(){self.task_details=None;return;}
        use eframe::egui;let mut changed=false;let mut close=false;
        ctx.show_viewport_immediate(egui::ViewportId::from_hash_of("fuyu-task-details"),egui::ViewportBuilder::default().with_title("浮屿 · 待办详情").with_inner_size([530.,610.]).with_taskbar(true),|ctx,_|{
            egui::CentralPanel::default().show(ctx,|ui|{egui::ScrollArea::vertical().show(ui,|ui|{
                let item=&mut self.todos.items[index];ui.heading("待办详情");changed|=ui.text_edit_singleline(&mut item.text).changed();
                changed|=ui.checkbox(&mut item.extra.important,"重要任务").changed();
                ui.label("到期时间（YYYY-MM-DD HH:MM，回车或离开输入框后保存）");
                let draft_id=egui::Id::new(("task-due-draft",index));let mut due=ctx.data_mut(|d|d.get_temp::<String>(draft_id)).unwrap_or_else(||item.due.clone().unwrap_or_default());
                let response=ui.text_edit_singleline(&mut due);ctx.data_mut(|d|d.insert_temp(draft_id,due.clone()));
                if response.lost_focus() || (response.has_focus()&&ui.input(|i|i.key_pressed(egui::Key::Enter))){if due.trim().is_empty(){item.due=None;changed=true;}else if NaiveDateTime::parse_from_str(due.trim(),"%Y-%m-%d %H:%M").is_ok(){item.due=Some(due.trim().into());item.reminded=false;changed=true;}}
                if !due.trim().is_empty()&&NaiveDateTime::parse_from_str(due.trim(),"%Y-%m-%d %H:%M").is_err(){ui.colored_label(egui::Color32::RED,"请填写完整日期与时间");}
                ui.horizontal(|ui|{ui.label("重复");for(key,label)in[("","不重复"),("daily","每天"),("weekly","每周"),("monthly","每月"),("yearly","每年")]{if ui.selectable_value(&mut item.extra.recurrence,key.into(),label).changed(){item.extra.last_reminded_due=None;item.done_on=None;if key!="weekly"{item.repeat=0;}changed=true;}}});
                if item.extra.recurring(){ui.horizontal(|ui|{ui.label("间隔");changed|=ui.add(egui::DragValue::new(&mut item.extra.interval).range(1..=365)).changed();ui.small("按所选周期计算；短月自动取月末");});}
                if item.extra.recurrence=="weekly"{ui.horizontal(|ui|{for(i,label)in crate::model::WEEKDAY_NAMES.iter().enumerate(){let mut on=item.repeat&(1<<i)!=0;if ui.checkbox(&mut on,*label).changed(){if on{item.repeat|=1<<i}else{item.repeat&=!(1<<i)}changed=true;}}});}
                ui.horizontal(|ui|{ui.label("提前提醒（分钟）");changed|=ui.add(egui::DragValue::new(&mut item.extra.reminder_offset).range(0..=10080)).changed();if ui.button("15 分钟后再提醒").clicked(){item.extra.snoozed_until=Some((chrono::Local::now()+Duration::minutes(15)).format("%Y-%m-%d %H:%M").to_string());item.reminded=false;changed=true;}});
                ui.separator();ui.label("备注");changed|=ui.add(egui::TextEdit::multiline(&mut item.extra.notes).desired_rows(5).desired_width(f32::INFINITY)).changed();
                ui.label("子任务");let mut remove=None;for(i,step)in item.extra.steps.iter_mut().enumerate(){ui.horizontal(|ui|{changed|=ui.checkbox(&mut step.done,"").changed();changed|=ui.text_edit_singleline(&mut step.text).changed();if ui.small_button("移除").clicked(){remove=Some(i)}});}if let Some(i)=remove{item.extra.steps.remove(i);changed=true;}if ui.button("添加子任务").clicked(){item.extra.steps.push(Step::default());changed=true;}
                ui.separator();ui.label("附件路径（点击打开）");let mut remove=None;for(i,path)in item.extra.attachments.iter().enumerate(){ui.horizontal(|ui|{if ui.link(path.display().to_string()).clicked(){self.pending.push(crate::app::Cmd::Open(path.clone()));}if ui.small_button("移除记录").clicked(){remove=Some(i)}});}if let Some(i)=remove{item.extra.attachments.remove(i);changed=true;}ui.small("可以把文件拖入此窗口添加附件，不复制或删除原件。");for file in ctx.input(|i|i.raw.dropped_files.clone()){if let Some(path)=file.path{if !item.extra.attachments.contains(&path){item.extra.attachments.push(path);changed=true;}}}
                if ui.button("完成").clicked(){close=true;}if changed{ui.small("已自动保存");}
            });});if ctx.input(|i|i.viewport().close_requested()){close=true;}
        });if changed{self.todos.save();}if close{self.task_details=None;}
    }
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn extra_roundtrip_preserves_metadata(){let mut t=crate::model::TodoItem::new("task|title");t.extra.notes="notes\nsecond".into();t.extra.steps.push(Step{text:"step".into(),done:true});t.extra.recurrence="monthly".into();t.done_on=Some("2027-02-28".into());t.due=Some("2027-01-31 10:00".into());let list=crate::model::TodoList{items:vec![t.clone()]};let parsed=crate::model::TodoList::parse(&list.serialize());assert_eq!(parsed.items[0].extra,t.extra);assert_eq!(parsed.items[0].done_on,t.done_on);assert_eq!(parsed.items[0].text,t.text);}
 #[test]fn month_end_and_leap_day_repeat(){let e=Extra{recurrence:"monthly".into(),interval:1,..Default::default()};assert!(e.scheduled("2027-02-28",Some("2027-01-31 10:00"),0));assert!(!e.scheduled("2027-02-27",Some("2027-01-31 10:00"),0));let e=Extra{recurrence:"yearly".into(),interval:1,..Default::default()};assert!(e.scheduled("2029-02-28",Some("2028-02-29 10:00"),0));}
 #[test]fn offset_and_snooze_are_only_reported_once(){let mut t=crate::model::TodoItem::new("task");t.due=Some("2027-01-01 10:00".into());t.extra.reminder_offset=15;assert!(!t.reminder_due("2027-01-01 09:44"));assert!(t.reminder_due("2027-01-01 09:45"));t.mark_reminded("2027-01-01 09:45");assert!(!t.reminder_due("2027-01-01 10:00"));t.extra.snoozed_until=Some("2027-01-01 11:00".into());assert!(t.reminder_due("2027-01-01 11:00"));t.mark_reminded("2027-01-01 11:00");assert!(!t.reminder_due("2027-01-01 11:01"));}
}
