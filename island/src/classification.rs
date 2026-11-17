//! 从浮屿原 C# 分类规则迁移；保持细分类与具体应用入口优先。
use std::sync::OnceLock;
use std::path::{Path,PathBuf};
pub fn shortcut(path:&Path)->Option<(PathBuf,String)>{
 #[cfg(windows)]unsafe{
  use std::os::windows::ffi::OsStrExt;
  use windows::{core::{PCWSTR,Interface},Win32::{System::Com::{CoInitializeEx,CoUninitialize,CoCreateInstance,COINIT_APARTMENTTHREADED,CLSCTX_INPROC_SERVER,IPersistFile,STGM_READ},UI::Shell::{IShellLinkW,ShellLink}}};
  if !path.extension().is_some_and(|e|e.eq_ignore_ascii_case("lnk")){return None;}
  let initialized=CoInitializeEx(None,COINIT_APARTMENTTHREADED).is_ok();
  let result=(||{let link:IShellLinkW=CoCreateInstance(&ShellLink,None,CLSCTX_INPROC_SERVER).ok()?;let persist:IPersistFile=link.cast().ok()?;let p:Vec<u16>=path.as_os_str().encode_wide().chain(Some(0)).collect();persist.Load(PCWSTR(p.as_ptr()),STGM_READ).ok()?;let mut file=[0u16;32768];link.GetPath(&mut file,std::ptr::null_mut(),0).ok()?;let n=file.iter().position(|c|*c==0).unwrap_or(file.len());if n==0{return None;}let target=PathBuf::from(String::from_utf16_lossy(&file[..n]));let mut description=[0u16;2048];let _=link.GetDescription(&mut description);let n=description.iter().position(|c|*c==0).unwrap_or(description.len());Some((target,String::from_utf16_lossy(&description[..n])))})();
  if initialized{CoUninitialize();}result
 }
 #[cfg(not(windows))]{let _=path;None}
}
pub fn entry_category(name:&str,path:&Path)->String{
 let result=classify(name);if result!="其他应用"{return result;}
 if let Some((target,description))=shortcut(path){return classify(&format!("{} {} {}",target.file_name().unwrap_or_default().to_string_lossy(),target.parent().and_then(|p|p.file_name()).unwrap_or_default().to_string_lossy(),description));}result
}
type Rule=(&'static str,Vec<&'static str>,Vec<regex::Regex>,Vec<&'static str>);
pub fn classify(name:&str)->String{
 static RULES:OnceLock<Vec<Rule>>=OnceLock::new();
 let rules=RULES.get_or_init(||{
  let raw:Vec<(&str,Vec<&str>,Vec<&str>,Vec<&str>)>=vec![("安装与卸载",vec!["卸载","uninstall","unins000","安装程序","安装向导","install additional","setup.exe"],vec![],vec![]),
("文档与帮助",vec!["说明","說明","操作手冊","帮助","使用问题","意见反馈","常见问题","使用帮助","release notes","documentation","frequently asked","support center","新鮮事","官网","官方网站","website","打字入门","细胞词库","皮肤下载"],vec!["help|faqs?|readme"],vec![]),
("配置与维护",vec!["语言首选项","设置向导","配置工具","检查升级","输入法修复","输入法管理","皮肤盒子"],vec!["settings|configure|configuration wizard"],vec!["设置"]),
("音乐与音频",vec!["qq音乐","网易云","酷狗","酷我","听歌识曲","音乐","spotify","foobar","audacity","录音","音频","music","voice recorder"],vec![],vec![]),
("游戏与平台",vec!["qq游戏","wegame","steam","epic games","rockstar","riot client","league of legends","英雄联盟","米哈游","鹰角","暴雪","战网","守望先锋","red dead redemption","底特律","mumu","模拟器","游戏"],vec!["battle.net|gog"],vec![]),
("AI助手",vec!["officeplus ai","chatgpt","chatbox","deepseek","ollama","workbuddy","千问","豆包","通义","claude","copilot","gemini","lm studio","腾讯元宝"],vec![],vec![]),
("文档与写作",vec!["scrivener","写作","wordpad","notepad","记事本"],vec!["winword|word|writer|typora"],vec![]),
("表格与数据",vec!["excel","表格","spreadsheet"],vec!["et.exe|spss|stata|matlab"],vec![]),
("演示与办公",vec!["powerpoint","演示","wps office","officeplus"],vec!["powerpnt|keynote"],vec![]),
("笔记与知识",vec!["onenote","notion","obsidian","sticky notes","便笺","笔记","思源","语雀","logseq","印象笔记"],vec![],vec![]),
("PDF与阅读",vec!["pdf","acrobat","xps viewer","calibre","阅读","阅读器","foxit"],vec![],vec![]),
("邮件与日程",vec!["outlook","thunderbird","邮件","日历","calendar","mail.exe"],vec![],vec![]),
("浏览器",vec!["chrome","msedge","microsoft edge","firefox","浏览器","brave","vivaldi"],vec!["opera|browser"],vec![]),
("聊天与会议",vec!["wechat","weixin","微信","钉钉","飞书","teams","slack","discord","telegram","zoom","腾讯会议","tim.exe"],vec!["qq|qq.exe"],vec![]),
("版本控制",vec!["github desktop","sourcetree","tortoisegit","fork.exe","git gui"],vec!["git.exe"],vec!["git"]),
("终端与运行时",vec!["command prompt","cmd.exe","powershell","terminal","msys2","mingw","clang64","ucrt64","node.js","python","anaconda","miniconda","git bash","git cmd"],vec![],vec![]),
("数据库工具",vec!["dbeaver","datagrip","navicat","heidisql","mysql workbench","sql server management","数据库","odbc data sources","pgadmin"],vec![],vec![]),
("编程与IDE",vec!["visual studio","vscode","pycharm","intellij","android studio","webstorm","rider","sublime text","notepad++","eclipse","docker","postman","insomnia","cursor.exe"],vec!["code.exe|zed.exe|unity|godot|idea"],vec![]),
("图像与设计",vec!["photoshop","illustrator","figma","blender","画图","绘图","照片","相机","camera","lightroom","inkscape","gimp","affinity","canva","sketch"],vec![],vec![]),
("视频与剪辑",vec!["哔哩哔哩","bilibili","剪映","视频","potplayer","vlc","media player","premiere","davinci","obs studio","mpv","capcut","after effects"],vec![],vec![]),
("网盘与传输",vec!["onedrive","dropbox","网盘","百度网盘","阿里云盘","坚果云","迅雷","filezilla","winscp","下载器","qbittorrent"],vec![],vec![]),
("网络与远程",vec!["todesk","蒲公英","星辰","vpn","远程","remote desktop","anydesk","teamviewer","向日葵","uu加速","clash","wireguard","tailscale","zerotier","mstsc"],vec![],vec![]),
("文件与压缩",vec!["winrar","7-zip","bandizip","压缩","file explorer","explorer.exe","资源管理器","total commander","everything","directory opus"],vec![],vec![]),
("辅助功能",vec!["livecaptions","magnify","narrator","on-screen keyboard","voiceaccess","accessibility","放大镜","讲述人","屏幕键盘","语音访问","实时字幕","osk.exe"],vec![],vec![]),
("系统管理",vec!["administrative tools","control panel","task manager","component services","computer management","dfrgui","disk cleanup","event viewer","iscsi","memory diagnostics","performance monitor","recoverydrive","registry editor","resource monitor","services","system configuration","system information","task scheduler","firewall","nvidia app","steps recorder","character map","计算器","控制面板","任务管理","e管家","应用商店","calculator","系统","设置"],vec!["run|run.exe"],vec![])];
  raw.into_iter().map(|(label,words,patterns,exact)|(label,words,patterns.into_iter().map(|p|regex::Regex::new(&format!(r"(?i)(?:^|[^a-z0-9])(?:{})(?:$|[^a-z0-9])",p)).unwrap()).collect(),exact)).collect()
 });
 let value=name.to_lowercase();for(label,words,patterns,exact)in rules{if words.iter().any(|w|value.contains(&w.to_lowercase()))||patterns.iter().any(|p|p.is_match(name))||exact.iter().any(|e|value==e.to_lowercase()){return (*label).into();}}
 "其他应用".into()
}
#[cfg(test)]mod tests{#[test]fn original_regression_cases(){for(name,expected)in[("QQ音乐","音乐与音频"),("QQ游戏","游戏与平台"),("QQ","聊天与会议"),("卸载微信","安装与卸载"),("Node.js documentation","文档与帮助"),("Office 语言首选项","配置与维护"),("Codecs","其他应用"),("Git GUI","版本控制"),("Visual Studio Code","编程与IDE")]{assert_eq!(super::classify(name),expected,"{name}");}}}
