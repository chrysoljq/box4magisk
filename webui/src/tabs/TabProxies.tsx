import { useCallback, useMemo, useState } from 'react';
import { Activity, Plus, RefreshCw, Server, ServerOff, TriangleAlert, ZapOff } from 'lucide-react';
import { cn } from '@/lib/cn';
import { ProxyGroupCard } from '@/features/proxies/components/ProxyGroupCard';
import { ProxyProviderCard } from '@/features/proxies/components/ProxyProviderCard';
import { useProxyData } from '@/features/proxies/hooks/useProxyData';
import { useProxyPrefs } from '@/features/proxies/hooks/useProxyPrefs';
import { notify } from '@/lib/bridge';
import { type NodeSortType, type ProviderCardModel, type TabProxiesProps } from '@/features/proxies/types';

const GROUP_TYPES = ['Selector', 'URLTest', 'Fallback', 'LoadBalance'];

export function TabProxies({ status }: TabProxiesProps) {
  const {
    viewType,
    setViewType,
    expanded,
    setExpanded,
    expandedProviders,
    setExpandedProviders,
    groupSorts,
    setGroupSorts,
  } = useProxyPrefs();

  const {
    proxies,
    providers,
    subscriptions,
    latencies,
    loading,
    apiError,
    testingOwners,
    testingNodes,
    updatingProvider,
    fetchInitialData,
    handleSelectNode,
    handleUpdateProvider,
    handleTestProvider,
    handleTestGroup,
    handleSaveSubscription,
    handleRemoveSubscription,
    handleRefreshSubscription,
  } = useProxyData(status);
  const isMihomo = status.bin_name === 'mihomo';
  const isSingbox = status.bin_name === 'sing-box';

  const [editor, setEditor] = useState<{ open: boolean, originalName: string | null, nextName: string, url: string, type: 'remote' | 'local' } | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<string | null>(null);

  const toggleExpand = useCallback((groupName: string) => {
    setExpanded(prev => ({ ...prev, [groupName]: !prev[groupName] }));
  }, [setExpanded]);

  const toggleProviderExpand = useCallback((name: string) => {
    setExpandedProviders(prev => ({ ...prev, [name]: !prev[name] }));
  }, [setExpandedProviders]);

  const toggleGroupSort = useCallback((e: React.MouseEvent, groupName: string) => {
    e.stopPropagation();
    const orders: NodeSortType[] = ['default', 'latency', 'name'];
    setGroupSorts(prev => {
      const current = prev[groupName] || 'default';
      const next = orders[(orders.indexOf(current) + 1) % orders.length];
      return { ...prev, [groupName]: next };
    });
  }, [setGroupSorts]);

  const proxyGroups = useMemo(() => {
  if (!proxies) return [];
  const globalOrder = proxies.GLOBAL?.all || [];
  
  const orderMap = new Map(globalOrder.map((name, index) => [name, index]));
  
  return Object.keys(proxies)
    .filter(name => GROUP_TYPES.includes(proxies[name].type))
    .sort((a, b) => {
      const idxA = orderMap.get(a) ?? 999;
      const idxB = orderMap.get(b) ?? 999;
      return idxA - idxB;
    });
}, [proxies]);

  const providerList = useMemo<ProviderCardModel[]>(() => {
    const runtimeEntries = Object.entries(providers || {}).filter(([, provider]) => provider.vehicleType !== 'Compatible');
    const runtimeMap = new Map(runtimeEntries);
    const subscriptionMap = new Map(subscriptions.map(subscription => [subscription.name, subscription]));
    const names = Array.from(new Set([...subscriptionMap.keys(), ...runtimeEntries.map(([name]) => name)]));

    return names.map(name => {
      const runtimeProvider = runtimeMap.get(name);
      const subscription = subscriptionMap.get(name);
      return {
        name,
        provider: runtimeProvider
          ? {
            ...runtimeProvider,
            updatedAt: runtimeProvider.updatedAt || subscription?.update_time || '',
          }
          : {
            name,
            type: 'Subscription',
            vehicleType: isSingbox
              ? (subscription?.status === 'ok' ? 'Subscription' : 'Needs Attention')
              : 'Configured',
            updatedAt: subscription?.update_time || '',
            proxies: subscription?.nodes || [],
          },
        subscription,
        hasRuntimeProvider: Boolean(runtimeProvider),
      };
    });
  }, [isSingbox, providers, subscriptions]);

  const getProviderBadgeLabel = useCallback((subscriptionType?: string, fallback?: string) => {
    if (!isSingbox) return fallback || 'Subscription';
    if (subscriptionType === 'local') return 'Local';
    if (subscriptionType === 'remote') return 'Remote';
    if (fallback && fallback !== 'Subscription') return fallback;
    return 'Remote';
  }, [isSingbox]);

  const openSubscriptionEditor = useCallback((currentName?: string, currentUrl?: string, currentType?: 'remote' | 'local') => {
    setEditor({
      open: true,
      originalName: currentName || null,
      nextName: currentName || `sub_${Date.now()}`,
      url: currentUrl || '',
      type: currentType || 'remote',
    });
  }, []);

  const handleEditSubscription = useCallback((e: React.MouseEvent, name: string, url?: string, type?: 'remote' | 'local') => {
    e.stopPropagation();
    void openSubscriptionEditor(name, url, type);
  }, [openSubscriptionEditor]);

  const handleAddSubscription = useCallback((type: 'remote' | 'local' = 'remote') => {
    void openSubscriptionEditor(undefined, undefined, type);
  }, [openSubscriptionEditor]);

  const handleUpdateCard = useCallback(async (e: React.MouseEvent, name: string, url?: string, hasRuntimeProvider?: boolean) => {
    if (hasRuntimeProvider) {
      handleUpdateProvider(e, name);
      return;
    }
    e.stopPropagation();
    if (!url) return;
    try {
      await handleRefreshSubscription(name, url);
    } catch (error) {
      notify(`刷新失败: ${error instanceof Error ? error.message : String(error)}`);
    }
  }, [handleRefreshSubscription, handleUpdateProvider]);

  const handleDeleteSubscription = useCallback((e: React.MouseEvent, name: string) => {
    e.stopPropagation();
    setDeleteTarget(name);
  }, []);

  const confirmDeleteSubscription = useCallback(async () => {
    if (!deleteTarget) return;
    try {
      await handleRemoveSubscription(deleteTarget);
      notify('订阅已删除');
      setDeleteTarget(null);
    } catch (error) {
      notify(`删除失败: ${error instanceof Error ? error.message : String(error)}`);
    }
  }, [deleteTarget, handleRemoveSubscription]);

  if (!status.running) {
    return (
      <div className="h-full flex flex-col items-center justify-center text-slate-400 px-8 text-center pb-20 animate-in fade-in">
        <Server size={48} className="opacity-20 mb-4" />
        <p className="text-sm">服务未运行<br />请先启动核心</p>
      </div>
    );
  }

  if (apiError) {
    return (
      <div className="h-full flex flex-col items-center justify-center px-8 text-center pb-20 animate-in fade-in">
        <div className="w-20 h-20 bg-rose-50 dark:bg-rose-500/10 rounded-full flex items-center justify-center mb-6">
          <ServerOff size={40} className="text-rose-500" strokeWidth={1.5} />
        </div>
        <h2 className="text-lg font-bold text-slate-900 dark:text-slate-100 mb-2">后端 API 连接失败</h2>
        <p className="text-sm text-slate-500 dark:text-slate-400 mb-8 leading-relaxed">
          无法连接到核心代理面板接口。<br />请检查代理核心是否已成功启动。
        </p>
        <button
          onClick={() => { void fetchInitialData(); }}
          className="flex items-center space-x-2 bg-[#3b82f6] hover:bg-blue-600 text-white px-6 py-3 rounded-full text-sm font-bold shadow-[0_4px_16px_rgba(59,130,246,0.3)] active:scale-95 transition-all"
        >
          <RefreshCw size={18} />
          <span>重新连接</span>
        </button>
      </div>
    );
  }

  if (loading || !proxies) {
    return (
      <div className="h-full flex flex-col items-center justify-center pb-20 animate-pulse text-slate-400">
        <Activity size={28} className="text-indigo-500 mb-4" />
        <span className="text-sm font-medium">获取代理信息中...</span>
      </div>
    );
  }

  return (
    <div className="px-4 pb-6 pt-2 space-y-4 animate-in fade-in slide-in-from-bottom-2 duration-300">

      <div className="flex bg-slate-200/60 dark:bg-slate-800/60 p-1 rounded-2xl mb-4">
        <button
          onClick={() => setViewType('proxies')}
          className={cn(
            'flex-1 py-2.5 rounded-xl text-[13px] font-bold transition-all flex items-center justify-center space-x-2',
            viewType === 'proxies' ? 'bg-[#3b82f6] text-white shadow-md' : 'text-slate-500 dark:text-slate-400',
          )}
        >
          <span>代理组</span>
          <span className={cn('px-1.5 py-0.5 rounded-md text-[10px]', viewType === 'proxies' ? 'bg-white/20' : 'bg-slate-300/50 dark:bg-slate-700')}>
            {proxyGroups.length}
          </span>
        </button>
        <button
          onClick={() => setViewType('providers')}
          className={cn(
            'flex-1 py-2.5 rounded-xl text-[13px] font-bold transition-all flex items-center justify-center space-x-2',
            viewType === 'providers' ? 'bg-[#3b82f6] text-white shadow-md' : 'text-slate-500 dark:text-slate-400',
          )}
        >
          <span>代理集合</span>
          <span className={cn('px-1.5 py-0.5 rounded-md text-[10px]', viewType === 'providers' ? 'bg-white/20' : 'bg-slate-300/50 dark:bg-slate-700')}>
            {providerList.length}
          </span>
        </button>
      </div>

      {viewType === 'proxies' && proxyGroups.map(groupName => (
        <ProxyGroupCard
          key={groupName}
          groupName={groupName}
          group={proxies[groupName]}
          proxies={proxies}
          latencies={latencies}
          testingOwners={testingOwners}
          testingNodes={testingNodes}
          isExpanded={expanded[groupName]}
          sortType={groupSorts[groupName] || 'default'}
          onToggleExpand={toggleExpand}
          onToggleSort={toggleGroupSort}
          onTestGroup={handleTestGroup}
          onSelectNode={handleSelectNode}
        />
      ))}

      {viewType === 'providers' && providerList.map(({ name, provider, subscription, hasRuntimeProvider }) => (
        <ProxyProviderCard
          key={name}
          name={name}
          provider={provider}
          providerBadgeLabel={getProviderBadgeLabel(subscription?.type, provider.vehicleType)}
          subscriptionStatus={subscription?.status}
          subscriptionWarnings={subscription?.warnings}
          latencies={latencies}
          testingOwners={testingOwners}
          testingNodes={testingNodes}
          isExpanded={expandedProviders[name]}
          isUpdating={updatingProvider === name}
          canManageSubscription={isMihomo || isSingbox}
          hasRuntimeProvider={hasRuntimeProvider}
          canRefreshProvider={hasRuntimeProvider || (isSingbox ? subscription?.type === 'remote' : Boolean(subscription?.url))}
          updateTitle={
            isSingbox && subscription?.type === 'local'
              ? '本地订阅不支持刷新'
              : (hasRuntimeProvider ? '更新 provider' : '刷新订阅缓存并重生成配置')
          }
          onToggleExpand={toggleProviderExpand}
          onUpdate={(e, providerName) => void handleUpdateCard(e, providerName, subscription?.url, hasRuntimeProvider)}
          onTest={handleTestProvider}
          onTestNode={handleTestGroup}
          onEditSubscription={subscription ? ((e, providerName) => handleEditSubscription(e, providerName, subscription.url, subscription.type === 'local' ? 'local' : 'remote')) : undefined}
          onRemoveSubscription={subscription ? ((e, providerName) => void handleDeleteSubscription(e, providerName)) : undefined}
        />
      ))}

      {viewType === 'providers' && (isMihomo || isSingbox) && (
        <div className="grid grid-cols-1 gap-3">
          <button
            onClick={() => handleAddSubscription()}
            className="flex w-full items-center justify-center gap-2 rounded-3xl border border-dashed border-sky-200 bg-sky-50/70 px-5 py-5 text-sm font-semibold text-sky-600 transition-colors hover:bg-sky-100 dark:border-sky-900/70 dark:bg-sky-950/20 dark:text-sky-300 dark:hover:bg-sky-950/30"
          >
            <Plus size={18} />
            <span>新增订阅</span>
          </button>
        </div>
      )}

      {viewType === 'providers' && providerList.length === 0 && (
        <div className="flex flex-col items-center justify-center py-16 text-slate-400">
          <ZapOff size={40} className="opacity-20 mb-3" />
          <p className="text-sm">{isMihomo || isSingbox ? '还没有已配置的订阅' : '未发现活跃的外部代理集合'}</p>
        </div>
      )}

      {editor?.open && (
        <>
          <div className="fixed inset-0 z-50 bg-slate-950/60 transition-opacity backdrop-blur-sm animate-in fade-in" onClick={() => setEditor(null)} />
          <div className="fixed left-1/2 top-1/2 z-50 w-[90%] max-w-sm -translate-x-1/2 -translate-y-1/2 rounded-3xl border border-slate-200/80 bg-white/95 p-6 shadow-xl backdrop-blur dark:border-slate-800 dark:bg-slate-900/95 animate-in zoom-in-95 duration-200">
            <h3 className="text-lg font-bold text-slate-900 dark:text-slate-100 mb-4">{editor.originalName ? '编辑订阅' : '新增订阅'}</h3>
            <div className="space-y-4">
              {isSingbox && (
                <div>
                  <label className="text-xs font-semibold uppercase tracking-wider text-slate-500 mb-1.5 block">订阅类型</label>
                  <div className="grid grid-cols-2 gap-2 rounded-2xl bg-slate-100/80 p-1 dark:bg-slate-800/80">
                    {(['remote', 'local'] as const).map(type => (
                      <button
                        key={type}
                        type="button"
                        onClick={() => setEditor({ ...editor, type })}
                        className={cn(
                          'rounded-xl px-3 py-2 text-sm font-semibold transition-colors',
                          editor.type === type
                            ? 'bg-white text-sky-600 shadow-sm dark:bg-slate-900 dark:text-sky-300'
                            : 'text-slate-500 dark:text-slate-400'
                        )}
                      >
                        {type === 'remote' ? '远程订阅' : '本地订阅'}
                      </button>
                    ))}
                  </div>
                </div>
              )}
              <div>
                <label className="text-xs font-semibold uppercase tracking-wider text-slate-500 mb-1.5 block">订阅名称</label>
                <input
                  type="text"
                  value={editor.nextName}
                  onChange={(e) => setEditor({ ...editor, nextName: e.target.value })}
                  className="w-full bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl px-3 py-2 text-sm text-slate-800 dark:text-slate-200 focus:outline-none focus:border-sky-500"
                />
              </div>
              <div>
                <label className="text-xs font-semibold uppercase tracking-wider text-slate-500 mb-1.5 block">{editor.type === 'local' ? '本地路径' : '订阅链接'}</label>
                <textarea
                  value={editor.url}
                  onChange={(e) => setEditor({ ...editor, url: e.target.value })}
                  placeholder={editor.type === 'local' ? '例如: /data/adb/box/sing-box/subscriptions/demo.txt' : ''}
                  className="w-full h-24 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl px-3 py-2 text-sm text-slate-800 dark:text-slate-200 focus:outline-none focus:border-sky-500 resize-none break-all"
                />
              </div>
            </div>
            <div className="mt-6 flex gap-3">
              <button
                onClick={() => setEditor(null)}
                className="flex-1 py-2.5 rounded-xl font-semibold text-slate-600 dark:text-slate-400 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 transition-colors"
              >
                取消
              </button>
              <button
                onClick={async () => {
                  try {
                    await handleSaveSubscription(editor.originalName, editor.nextName.trim(), editor.url.trim(), editor.type);
                    setEditor(null);
                  } catch(e) {
                    notify(`订阅保存失败: ${e instanceof Error ? e.message : String(e)}`);
                  }
                }}
                disabled={!editor.nextName.trim() || !editor.url.trim()}
                className="flex-1 py-2.5 rounded-xl font-semibold text-white bg-sky-500 hover:bg-sky-600 disabled:opacity-50 transition-colors shadow-sm"
              >
                保存
              </button>
            </div>
          </div>
        </>
      )}

      {deleteTarget && (
        <>
          <div className="fixed inset-0 z-50 bg-slate-950/60 transition-opacity backdrop-blur-sm animate-in fade-in" onClick={() => setDeleteTarget(null)} />
          <div className="fixed left-1/2 top-1/2 z-50 w-[90%] max-w-sm -translate-x-1/2 -translate-y-1/2 rounded-3xl border border-rose-200/70 bg-white/95 p-6 shadow-xl backdrop-blur dark:border-rose-900/60 dark:bg-slate-900/95 animate-in zoom-in-95 duration-200">
            <div className="flex items-start gap-3">
              <div className="mt-0.5 flex h-10 w-10 shrink-0 items-center justify-center rounded-2xl bg-rose-50 text-rose-500 dark:bg-rose-500/10 dark:text-rose-300">
                <TriangleAlert size={18} />
              </div>
              <div className="min-w-0">
                <h3 className="text-lg font-bold text-slate-900 dark:text-slate-100">删除订阅</h3>
                <p className="mt-1 text-sm leading-6 text-slate-600 dark:text-slate-300">
                  确定删除订阅
                  <span className="mx-1 font-semibold text-slate-900 dark:text-slate-100">「{deleteTarget}」</span>
                  吗？删除后将从当前核心配置中移除。
                </p>
              </div>
            </div>
            <div className="mt-6 flex gap-3">
              <button
                onClick={() => setDeleteTarget(null)}
                className="flex-1 py-2.5 rounded-xl font-semibold text-slate-600 dark:text-slate-400 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 transition-colors"
              >
                取消
              </button>
              <button
                onClick={() => { void confirmDeleteSubscription(); }}
                className="flex-1 py-2.5 rounded-xl font-semibold text-white bg-rose-500 hover:bg-rose-600 transition-colors shadow-sm"
              >
                确认删除
              </button>
            </div>
          </div>
        </>
      )}
    </div>
  );
}
