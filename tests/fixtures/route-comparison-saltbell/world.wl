include "shared.wl"
world lightship as "盐钟灯船与票据档案"
entity bell kind artifact as "盐钟：一个等待决定的漫长中文名字"
character cenhe as "岑禾"
character xiyao as "汐遥"
tag quiet as "沉默"
tag ringing as "鸣响"
tag sealed as "封存"
tag entered as "已登船"
tag observed as "已核对"
tag logged as "已登记"
tag departed as "已离开档案间"
tag finished as "自然完结"
tag archived as "归档"
tag never as "未加入的标签"
state bell_state on entity bell with quiet as "盐钟状态"
state ledger_state on world lightship with [] as "票据过程"
let receipt = 0
let repeats = 0

event embark as "第一章：乘着夜色抵达盐钟灯船"
  effect on enter
    become ledger_state add entered as "入口效果"
  岑禾与汐遥登上灯船。
  -> decision

event decision as "第二章：盐钟的决定"
  这一次如何处理盐钟？
  choice "敲响盐钟"
    become bell_state with ringing as "决定鸣响"
    set receipt = 0
    -> archive
  choice "把盐钟封存"
    become bell_state with sealed as "决定封存"
    set receipt = 7
    -> archive
  choice "敲响后再次写下同样决定"
    become bell_state with ringing as "决定鸣响"
    become bell_state with ringing as "决定鸣响"
    set receipt = 0
    -> archive

event archive as "第三章：票据档案的重复核对"
  effect on enter
    become ledger_state add observed as "档案入口效果"
  effect on exit
    become ledger_state add departed as "档案离开效果"
  call stamp_log()
  当前票据编号为{receipt}。
  choice "封卷"
    -> closure
  choice "再校对一次"
    set repeats = repeats + 1
    -> archive

event closure as "第四章：自然收束"
  effect on done
    become ledger_state add finished as "自然完成效果"
  effect on exit
    become ledger_state add archived as "最终离开效果"
  盐钟记录已归档，票据编号为{receipt}。
