import React, { useState } from 'react';
import { Activity, ChevronDown, Clock, Database, Pencil, RefreshCw, Server, Trash2, Zap, MoreVertical } from 'lucide-react';
import type { ProxyProvider } from '@/lib/clash';
import { cn } from '@/lib/cn';
import { formatBytes, formatDate, formatRelativeTime, getLatencyStyle } from '../utils';

interface ProxyProviderCardProps {
  name: string;
  provider: ProxyProvider;
  providerBadgeLabel?: string;
  subscriptionStatus?: string;
  subscriptionWarnings?: string[];
  latencies: Record<string, number>;
  testingOwners: Record<string, number>;
  testingNodes: Record<string, number>;
  isExpanded: boolean;
  isUpdating: boolean;
  canManageSubscription?: boolean;
  hasRuntimeProvider?: boolean;
  canRefreshProvider?: boolean;
  updateTitle?: string;
  onToggleExpand: (name: string) => void;
  onUpdate: (e: React.MouseEvent, name: string) => void;
  onTest: (e: React.MouseEvent, name: string) => void;
  onTestNode: (e: React.MouseEvent, groupName: string, nodes: string[]) => void;
  onEditSubscription?: (e: React.MouseEvent, name: string) => void;
  onRemoveSubscription?: (e: React.MouseEvent, name: string) => void;
}

export const ProxyProviderCard = React.memo((props: ProxyProviderCardProps) => {
  const {
    name,
    provider,
    providerBadgeLabel,
    subscriptionStatus,
    subscriptionWarnings,
    latencies,
    testingOwners,
    testingNodes,
    isExpanded,
    isUpdating,
    canManageSubscription,
    hasRuntimeProvider = true,
    canRefreshProvider = true,
    updateTitle,
    onToggleExpand,
    onUpdate,
    onTest,
    onTestNode,
    onEditSubscription,
    onRemoveSubscription,
  } = props;
  const isTesting = Boolean(testingOwners[`provider:${name}`]);
  const [menuOpen, setMenuOpen] = useState(false);

  return (
    <div className={cn(
      "bg-white dark:bg-slate-900 rounded-3xl p-5 shadow-sm border border-slate-100 dark:border-slate-800 transition-colors animate-in fade-in slide-in-from-bottom-2",
      menuOpen && "relative z-50"
    )}>
      <div className="relative cursor-pointer group-card" onClick={() => onToggleExpand(name)}>
        <div className="pr-[130px]">
          <div className="flex items-center space-x-2">
            <h3 className="font-bold text-[17px] text-slate-900 dark:text-slate-100 truncate leading-tight">{name}</h3>
            <span className="text-[10px] text-indigo-600 dark:text-indigo-400 bg-indigo-50 dark:bg-indigo-500/10 px-2 py-0.5 rounded-full font-bold uppercase tracking-wider shrink-0">
              {providerBadgeLabel || provider.vehicleType || 'Subscription'}
            </span>
          </div>
        </div>
        <div className="text-[12px] text-slate-400 dark:text-slate-500 mt-2.5 flex items-center space-x-4">
          <span className="flex items-center"><Activity size={12} className="mr-1" /> {formatRelativeTime(provider.updatedAt)}</span>
          <span className="font-semibold flex items-center"><Server size={12} className="mr-1" /> {provider.proxies?.length || 0} 节点</span>
        </div>

        <div className="absolute top-0 right-0 flex items-center space-x-0.5 text-slate-400 -mt-1.5 -mr-1.5">
          {canManageSubscription && (onEditSubscription || onRemoveSubscription) && (
            <div className="relative">
              <button
                onClick={(e) => { e.stopPropagation(); setMenuOpen(!menuOpen); }}
                className="p-2 rounded-xl transition-all hover:text-slate-600 hover:bg-slate-50 dark:hover:bg-slate-800"
                title="管理选项"
              >
                <MoreVertical size={18} />
              </button>
              {menuOpen && (
                <>
                  <div className="fixed inset-0 z-40" onClick={(e) => { e.stopPropagation(); setMenuOpen(false); }}></div>
                  <div className="absolute right-0 mt-2 w-32 rounded-xl bg-white dark:bg-slate-900 shadow-lg border border-slate-100 dark:border-slate-800 z-50 overflow-hidden py-1 animate-in slide-in-from-top-2 fade-in">
                    {onEditSubscription && (
                      <button
                        onClick={(e) => { e.stopPropagation(); setMenuOpen(false); onEditSubscription(e, name); }}
                        className="w-full flex items-center px-3 py-2.5 text-[13px] text-left text-slate-700 dark:text-slate-200 hover:bg-slate-50 dark:hover:bg-slate-800 transition-colors"
                      >
                        <Pencil size={14} className="mr-2 text-slate-400" /> 编辑信息
                      </button>
                    )}
                    {onRemoveSubscription && (
                      <button
                        onClick={(e) => { e.stopPropagation(); setMenuOpen(false); onRemoveSubscription(e, name); }}
                        className="w-full flex items-center px-3 py-2.5 text-[13px] text-left text-rose-500 hover:bg-rose-50 dark:hover:bg-rose-500/10 transition-colors border-t border-slate-100 dark:border-slate-800/50"
                      >
                        <Trash2 size={14} className="mr-2" /> 删除订阅
                      </button>
                    )}
                  </div>
                </>
              )}
            </div>
          )}
          <button
            onClick={e => onTest(e, name)}
            disabled={isTesting || !hasRuntimeProvider}
            className={cn('p-2 rounded-xl transition-all', isTesting ? 'text-indigo-500 animate-pulse' : 'hover:text-indigo-600 hover:bg-slate-50 dark:hover:bg-slate-800', !hasRuntimeProvider && 'opacity-40 cursor-not-allowed')}
            title={hasRuntimeProvider ? '测速' : '运行态 provider 不可用'}
          >
            <Zap size={18} />
          </button>
          <button
            onClick={e => onUpdate(e, name)}
            disabled={isUpdating || !canRefreshProvider}
            className={cn('p-2 rounded-xl transition-all', isUpdating ? 'bg-indigo-50 dark:bg-indigo-500/10 text-indigo-500' : 'hover:text-amber-500 hover:bg-slate-50 dark:hover:bg-slate-800', !canRefreshProvider && 'opacity-40 cursor-not-allowed')}
            title={updateTitle || (hasRuntimeProvider ? '更新 provider' : '运行态 provider 不可用')}
          >
            <RefreshCw size={18} className={isUpdating ? 'animate-spin' : ''} />
          </button>
          <ChevronDown size={20} className={cn('ml-1 transition-transform duration-300', isExpanded && 'rotate-180')} />
        </div>
      </div>

      {provider.subscriptionInfo && (() => {
        const sub = provider.subscriptionInfo;
        const used = (sub.Download || 0) + (sub.Upload || 0);
        const total = sub.Total || 0;
        const percent = total > 0 ? Math.min((used / total) * 100, 100) : 0;

        return (
          <div className="mt-4 pt-4 border-t border-slate-100 dark:border-slate-800/50 space-y-3">
            <div className="flex items-center justify-between text-[12px] text-slate-600 dark:text-slate-400">
              <div className="flex items-center space-x-1.5">
                <Database size={14} className="text-indigo-500" />
                <span className="font-medium">{formatBytes(used)} / {formatBytes(total)}</span>
              </div>
              <div className="flex items-center space-x-1.5 text-orange-500 font-medium">
                <Clock size={14} />
                <span>到期: {formatDate(sub.Expire)}</span>
              </div>
            </div>
            <div className="w-full h-1.5 bg-slate-100 dark:bg-slate-800 rounded-full overflow-hidden">
              <div className="h-full bg-indigo-500 rounded-full transition-all duration-700" style={{ width: `${percent}%` }} />
            </div>
          </div>
        );
      })()}

      {((subscriptionStatus && subscriptionStatus !== 'ok') || (subscriptionWarnings && subscriptionWarnings.length > 0)) && (
        <div className="mt-4 space-y-2 rounded-2xl border border-amber-200/70 bg-amber-50/70 px-3 py-3 text-[12px] text-amber-700 dark:border-amber-900/60 dark:bg-amber-950/20 dark:text-amber-300">
          {subscriptionStatus && subscriptionStatus !== 'ok' && (
            <div className="font-semibold">
              状态: {subscriptionStatus}
            </div>
          )}
          {subscriptionWarnings && subscriptionWarnings.slice(0, 3).map((warning, index) => (
            <div key={`${name}:warning:${index}`} className="leading-relaxed">
              {warning}
            </div>
          ))}
        </div>
      )}

      {isExpanded && provider.proxies && (
        <div className="mt-5 pt-5 border-t border-slate-100 dark:border-slate-800/50 grid grid-cols-2 gap-3 animate-in fade-in slide-in-from-top-2">
          {provider.proxies.length === 0 && (
            <div className="col-span-2 rounded-2xl border border-dashed border-slate-200 px-3 py-6 text-center text-[12px] text-slate-400 dark:border-slate-800 dark:text-slate-500">
              当前订阅已配置，但还没有可展示的运行态节点
            </div>
          )}
          {provider.proxies.map((node, index) => {
            const ms = latencies[node.name] || 0;
            const style = getLatencyStyle(ms);
            const isNodeTesting = Boolean(testingNodes[node.name]);
            return (
              <div key={index} className="flex flex-col px-3 py-2 rounded-2xl bg-slate-50/50 dark:bg-slate-800/40 border border-slate-100 dark:border-slate-800/60 transition-all hover:border-indigo-200 dark:hover:border-indigo-800/50 opacity-90">
                <div className="flex items-center space-x-1.5 mb-1.5">
                  <span className={cn('w-1.5 h-1.5 rounded-full shrink-0', style.bg)} />
                  <div className="text-[12px] font-semibold truncate text-slate-700 dark:text-slate-300 flex-1 leading-tight">{node.name}</div>
                </div>

                <div className="flex items-center justify-between w-full mt-1 pt-1 border-t border-slate-200/50 dark:border-slate-700/50 border-dashed">
                  <span className="text-[10px] text-slate-400 dark:text-slate-500 font-mono font-medium tracking-wide uppercase truncate pr-1">
                    {node.type || 'Unknown'}
                  </span>
                  <div
                    className={cn('text-[10px] font-mono font-bold bg-slate-100/80 dark:bg-slate-800 px-1.5 py-0.5 rounded transition-all cursor-pointer shrink-0 hover:bg-slate-200 dark:hover:bg-slate-700 active:scale-95', style.text)}
                    onClick={e => onTestNode(e, name, [node.name])}
                    title="点击测速"
                  >
                    {isNodeTesting ? '...' : (ms ? `${ms} ms` : '-')}
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
});

ProxyProviderCard.displayName = 'ProxyProviderCard';
