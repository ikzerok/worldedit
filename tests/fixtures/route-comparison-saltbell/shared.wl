fragment stamp_log()
  become ledger_state add logged as "写入登记"
  become ledger_state add logged as "写入登记"
  become ledger_state remove never as "移除不存在标签"
  return
