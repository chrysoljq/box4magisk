import { useState } from 'react';
import { RefreshCw, Save, Home, Layers, Settings, Server, Plus, Download, Link2, RotateCcw, Network } from 'lucide-react';
import { NavItem } from '@/components/ui';
import { useBoxController } from '@/hooks/useBoxController';
import { useTheme } from '@/hooks/useTheme';
import { boxBridge, notify, waitForJob } from '@/lib/bridge';
import { TabHome } from '@/tabs/TabHome';
import { TabProxies } from '@/tabs/TabProxies';
import { TabApps } from '@/tabs/TabApps';
import { TabAdvanced } from '@/tabs/TabAdvanced';
import '@/index.css';

export default function App() {
  const [activeTab, setActiveTab] = useState('home');
  const [menuOpen, setMenuOpen] = useState(false);
  useTheme();
  const {
    loading,
    status,
    config,
    appList,
    actionLoading,
    hasChanges,
    handleServiceAction,
    handleTproxyAction,
    handleToggle,
    handleChange,
    handleSaveAndApply,
    handleToggleAutoStart,
  } = useBoxController();

  if (loading) {
    return (
      <div className="flex min-h-dvh items-center justify-center bg-slate-50 dark:bg-slate-950">
        <div className="animate-spin text-indigo-500"><RefreshCw size={28} /></div>
      </div>
    );
  }

  const activeCore = config.bin_name || 'sing-box';
  const isCoreSupported = activeCore === 'mihomo' || activeCore === 'sing-box';

  const handleImportSub = () => {
    setMenuOpen(false);
    if (!isCoreSupported) return;
    const url = window.prompt('请输入订阅链接:');
    if (!url) return;
    const name = `sub_${Date.now()}`;
    if (activeCore === 'mihomo') {
      void boxBridge.addMihomoSubscription(name, url)
        .then(async job => {
          notify('订阅导入已转入后台');
          await waitForJob(job.job_id);
          notify('订阅添加成功');
        })
        .catch(e => notify(`导入失败: ${e instanceof Error ? e.message : e}`));
    } else {
      void boxBridge.addSingboxSubscription(name, url)
        .then(async job => {
          notify('订阅导入已转入后台');
          await waitForJob(job.job_id);
          notify('订阅添加成功');
        })
        .catch(e => notify(`导入失败: ${e instanceof Error ? e.message : e}`));
    }
  };

  const handleDownloadCore = () => {
    setMenuOpen(false);
    void boxBridge.downloadCores()
      .then(async job => {
        notify('核心下载已转入后台');
        await waitForJob(job.job_id, { timeoutMs: 300000 });
        notify('核心下载完成');
      })
      .catch(e => notify(`核心下载失败: ${e instanceof Error ? e.message : e}`));
  };

  const handleRestartTproxy = () => {
    setMenuOpen(false);
    void handleTproxyAction('restart');
  };

  const handleRestartCore = () => {
    setMenuOpen(false);
    void handleServiceAction('restart');
  };

  return (
    <div className="mx-auto h-dvh max-w-md overflow-hidden font-sans shadow-2xl transition-colors duration-300 bg-slate-50 text-slate-800 dark:bg-slate-950 dark:text-slate-200 relative">
      {/* Top Navigation */}
      <header className="bg-white/80 dark:bg-slate-900/80 backdrop-blur-md border-b border-slate-200 dark:border-slate-800 sticky top-0 z-30 transition-colors">
        <div className="px-5 py-3.5 flex items-center justify-between">
          <div className="flex items-center space-x-2.5">
            <div className={`w-2.5 h-2.5 rounded-full ${status.running ? 'bg-emerald-500 shadow-[0_0_8px_rgba(16,185,129,0.5)] animate-pulse' : 'bg-rose-500'}`} />
            <h1 className="text-lg font-bold text-slate-900 dark:text-slate-100 tracking-tight transition-colors">Box 控制台</h1>
            <div className="text-xs font-semibold text-slate-500 dark:text-slate-400 bg-slate-100 dark:bg-slate-800 px-2 py-0.5 rounded-md flex items-center transition-colors">
              {status.running ? `PID: ${status.pid}` : 'STOPPED'}
            </div>
          </div>

          <div className="relative">
            <button
              onClick={() => setMenuOpen(!menuOpen)}
              className="p-1.5 rounded-full bg-slate-100 dark:bg-slate-800 text-slate-500 dark:text-slate-400 hover:bg-slate-200 dark:hover:bg-slate-700 transition-colors"
              title="菜单"
            >
              <Plus size={18} className={`transition-transform duration-200 ${menuOpen ? 'rotate-45' : ''}`} />
            </button>

            {menuOpen && (
              <>
                <div className="fixed inset-0 z-40" onClick={() => setMenuOpen(false)}></div>
                <div className="absolute right-0 mt-2 w-48 rounded-xl bg-white dark:bg-slate-900 shadow-lg border border-slate-100 dark:border-slate-800 z-50 overflow-hidden py-1 animate-in slide-in-from-top-2 fade-in">
                  <button
                    onClick={handleRestartTproxy}
                    disabled={actionLoading !== null}
                    className="w-full flex items-center px-4 py-3 text-sm text-left text-slate-700 dark:text-slate-200 hover:bg-slate-50 dark:hover:bg-slate-800 transition-colors"
                  >
                    {actionLoading === 'tproxy-restart'
                      ? <RefreshCw size={16} className="mr-2 animate-spin" />
                      : <Network size={16} className="mr-2" />}
                    重启 TProxy
                  </button>
                  <button
                    onClick={handleRestartCore}
                    disabled={actionLoading !== null}
                    className="w-full flex items-center px-4 py-3 text-sm text-left text-slate-700 dark:text-slate-200 hover:bg-slate-50 dark:hover:bg-slate-800 transition-colors border-t border-slate-100 dark:border-slate-800/50"
                  >
                    {actionLoading === 'restart'
                      ? <RefreshCw size={16} className="mr-2 animate-spin" />
                      : <RotateCcw size={16} className="mr-2" />}
                    重启内核
                  </button>
                  <button
                    onClick={handleImportSub}
                    disabled={!isCoreSupported || actionLoading !== null}
                    className={`w-full flex items-center px-4 py-3 text-sm text-left transition-colors border-t border-slate-100 dark:border-slate-800/50 ${isCoreSupported ? 'text-slate-700 dark:text-slate-200 hover:bg-slate-50 dark:hover:bg-slate-800' : 'text-slate-400 dark:text-slate-600 opacity-60 cursor-not-allowed'}`}
                  >
                    <Link2 size={16} className="mr-2" />
                    导入订阅
                  </button>
                  <button
                    onClick={handleDownloadCore}
                    disabled={actionLoading !== null}
                    className="w-full flex items-center px-4 py-3 text-sm text-left text-slate-700 dark:text-slate-200 hover:bg-slate-50 dark:hover:bg-slate-800 transition-colors border-t border-slate-100 dark:border-slate-800/50"
                  >
                    <Download size={16} className="mr-2" />
                    下载核心 ({activeCore})
                  </button>
                </div>
              </>
            )}
          </div>
        </div>
      </header>

      {/* Main Content Area */}
      <main className="h-[calc(100dvh-53px)] overflow-y-auto pb-32 pt-2 scrollbar-hide">
        {activeTab === 'home' && <TabHome status={status} config={config} handleServiceAction={handleServiceAction} actionLoading={actionLoading} handleChange={handleChange} handleToggle={handleToggle} handleToggleAutoStart={handleToggleAutoStart} />}
        {activeTab === 'proxies' && <TabProxies status={status} />}
        {activeTab === 'apps' && <TabApps config={config} handleToggle={handleToggle} handleChange={handleChange} appList={appList} />}
        {activeTab === 'advanced' && <TabAdvanced status={status} config={config} handleToggle={handleToggle} handleChange={handleChange} />}
      </main>

      {/* Floating Save Button */}
      {hasChanges && (
        <div className="absolute bottom-16 right-6 z-40 animate-in slide-in-from-bottom-4 zoom-in duration-300">
          <button
            onClick={handleSaveAndApply}
            disabled={actionLoading === 'save'}
            className="bg-indigo-600 hover:bg-indigo-700 text-white px-5 py-3.5 rounded-full shadow-[0_4px_16px_rgba(79,70,229,0.4)] flex items-center space-x-2 font-bold active:scale-95 transition-all"
          >
            {actionLoading === 'save' ? <RefreshCw size={20} className="animate-spin" /> : <Save size={20} />}
            <span>{actionLoading === 'save' ? '保存中...' : '保存'}</span>
          </button>
        </div>
      )}

      {/* Bottom Navigation */}
      <nav className="absolute bottom-0 w-full bg-white dark:bg-slate-900 border-t border-slate-200 dark:border-slate-800 px-6 py-2 pb-safe flex justify-between items-center z-30 transition-colors">
        <NavItem icon={<Home size={24} />} label="首页" active={activeTab === 'home'} onClick={() => setActiveTab('home')} />
        <NavItem icon={<Server size={24} />} label="代理" active={activeTab === 'proxies'} onClick={() => setActiveTab('proxies')} />
        <NavItem icon={<Layers size={24} />} label="分流" active={activeTab === 'apps'} onClick={() => setActiveTab('apps')} />
        <NavItem icon={<Settings size={24} />} label="高级" active={activeTab === 'advanced'} onClick={() => setActiveTab('advanced')} />
      </nav>
    </div>
  );
}
