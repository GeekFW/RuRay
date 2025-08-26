/**
 * 错误常量映射表
 * 将后端错误常量映射到前端i18n键
 */

// TUN相关错误常量
export const TUN_ERRORS = {
  TUN_ERROR_ADMIN: 'tunConfig.errors.adminRequired',
  TUN_ERROR_NO_PROXY: 'tunConfig.errors.noProxyRunning',
  TUN_ERROR_START_FAILED: 'tunConfig.errors.startFailed',
  TUN_ERROR_STOP_FAILED: 'tunConfig.errors.stopFailed',
  TUN_ERROR_CONFIG_INVALID: 'tunConfig.errors.configInvalid',
  TUN_ERROR_DEVICE_BUSY: 'tunConfig.errors.deviceBusy'
}

// 服务器相关错误常量
export const SERVER_ERRORS = {
  SERVER_ERROR_DELETE_FAILED: 'serverList.messages.deleteFailedDesc',
  SERVER_ERROR_CONFIG_INVALID: 'serverList.messages.configInvalid',
  SERVER_ERROR_CONNECTION_FAILED: 'serverList.messages.connectionFailed'
}

// 代理相关错误常量
export const PROXY_ERRORS = {
  PROXY_ERROR_START_FAILED: 'statusBar.errors.proxyStartFailed',
  PROXY_ERROR_STOP_FAILED: 'statusBar.errors.proxyStopFailed',
  PROXY_ERROR_MODE_CHANGE_FAILED: 'statusBar.errors.modeChangeFailed'
}

/**
 * 根据错误消息获取对应的i18n键
 * @param {string} errorMessage - 后端返回的错误消息
 * @param {string} defaultKey - 默认的i18n键
 * @returns {string} 对应的i18n键
 */
export function getErrorI18nKey(errorMessage, defaultKey) {
  // 合并所有错误映射
  const allErrors = {
    ...TUN_ERRORS,
    ...SERVER_ERRORS,
    ...PROXY_ERRORS
  }
  
  // 查找匹配的错误常量
  for (const [errorConstant, i18nKey] of Object.entries(allErrors)) {
    if (errorMessage.includes(errorConstant)) {
      return i18nKey
    }
  }
  
  return defaultKey
}

/**
 * 专门用于TUN错误的映射函数
 * @param {string} errorMessage - 错误消息
 * @param {string} defaultKey - 默认键
 * @returns {string} i18n键
 */
export function getTunErrorI18nKey(errorMessage, defaultKey) {
  for (const [errorConstant, i18nKey] of Object.entries(TUN_ERRORS)) {
    if (errorMessage.includes(errorConstant)) {
      return i18nKey
    }
  }
  return defaultKey
}