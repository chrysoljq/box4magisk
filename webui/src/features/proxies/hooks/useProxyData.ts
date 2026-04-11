import type React from 'react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { ClashClient } from '@/lib/clash';
import { boxBridge, notify, waitForJob } from '@/lib/bridge';
import type { BoxConfig, BoxSubscription } from '@/types/box';
import type { ProviderMap, ProxyMap } from '../types';

function useIsMounted() {
  const isMounted = useRef(true);
  useEffect(() => {
    isMounted.current = true;
    return () => {
      isMounted.current = false;
    };
  }, []);
  return isMounted;
}

function getSubscriptionQueuedText(currentName: string | null) {
  return currentName ? '订阅更新任务' : '订阅新增任务';
}

function normalizeMode(mode: string | null | undefined) {
  return String(mode || '').trim();
}

function normalizeModeKey(mode: string | null | undefined) {
  return normalizeMode(mode).toLowerCase();
}

function getFallbackModes(binName?: string) {
  if (binName === 'sing-box') {
    return [];
  }
  return ['direct', 'rule', 'global'];
}

function buildAvailableModes(binName: string | undefined, currentMode: string, apiModeList?: string[]) {
  const modeMap = new Map<string, string>();
  const addMode = (mode: string | null | undefined) => {
    const normalized = normalizeMode(mode);
    const key = normalizeModeKey(normalized);
    if (!normalized || !key || modeMap.has(key)) return;
    modeMap.set(key, normalized);
  };

  apiModeList?.forEach(addMode);
  getFallbackModes(binName).forEach(addMode);

  addMode(currentMode);

  return Array.from(modeMap.values());
}

export function useProxyData(
  status: { running: boolean; bin_name?: string; clash_api_port: string; clash_api_secret: string },
  config?: BoxConfig
) {
  void config;
  const isMounted = useIsMounted();
  const lastFetchControllerRef = useRef<AbortController | null>(null);
  const fetchSequenceRef = useRef(0);
  const [proxies, setProxies] = useState<ProxyMap | null>(null);
  const [providers, setProviders] = useState<ProviderMap | null>(null);
  const [subscriptions, setSubscriptions] = useState<BoxSubscription[]>([]);
  const [latencies, setLatencies] = useState<Record<string, number>>({});
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [apiError, setApiError] = useState(false);
  const [apiErrorMessage, setApiErrorMessage] = useState<string | null>(null);
  const [currentMode, setCurrentMode] = useState<string>('rule');
  const [availableModes, setAvailableModes] = useState<string[]>(getFallbackModes(status.bin_name));
  const [testingOwners, setTestingOwners] = useState<Record<string, number>>({});
  const [testingNodes, setTestingNodes] = useState<Record<string, number>>({});
  const [updatingProviders, setUpdatingProviders] = useState<Record<string, number>>({});

  const client = useMemo(() => new ClashClient(status.clash_api_port, status.clash_api_secret), [status.clash_api_port, status.clash_api_secret]);

  const markTestingStart = useCallback((ownerKey: string, nodes: string[] = []) => {
    setTestingOwners(prev => ({ ...prev, [ownerKey]: (prev[ownerKey] || 0) + 1 }));
    if (nodes.length === 0) return;
    setTestingNodes(prev => {
      const next = { ...prev };
      nodes.forEach(node => {
        if (!node) return;
        next[node] = (next[node] || 0) + 1;
      });
      return next;
    });
  }, []);

  const markTestingEnd = useCallback((ownerKey: string, nodes: string[] = []) => {
    setTestingOwners(prev => {
      const current = prev[ownerKey] || 0;
      if (current <= 1) {
        const rest = { ...prev };
        delete rest[ownerKey];
        return rest;
      }
      return { ...prev, [ownerKey]: current - 1 };
    });

    if (nodes.length === 0) return;
    setTestingNodes(prev => {
      const next = { ...prev };
      nodes.forEach(node => {
        if (!node) return;
        const current = next[node] || 0;
        if (current <= 1) delete next[node];
        else next[node] = current - 1;
      });
      return next;
    });
  }, []);

  const markProviderUpdateStart = useCallback((name: string) => {
    setUpdatingProviders(prev => ({ ...prev, [name]: (prev[name] || 0) + 1 }));
  }, []);

  const markProviderUpdateEnd = useCallback((name: string) => {
    setUpdatingProviders(prev => {
      const current = prev[name] || 0;
      if (current <= 1) {
        const rest = { ...prev };
        delete rest[name];
        return rest;
      }
      return { ...prev, [name]: current - 1 };
    });
  }, []);

  const refreshManagedSubscriptions = useCallback(async (signal?: AbortSignal) => {
    if (status.bin_name !== 'mihomo' && status.bin_name !== 'sing-box') {
      if (isMounted.current) setSubscriptions([]);
      return [];
    }

    const data = await (status.bin_name === 'mihomo'
      ? boxBridge.mihomoSubscriptions()
      : boxBridge.singboxSubscriptionViews()) as BoxSubscription[];
    if (signal?.aborted || !isMounted.current) return data;
    setSubscriptions(Array.isArray(data) ? data : []);
    return data;
  }, [isMounted, status.bin_name]);

  const fetchInitialData = useCallback(async (
    signalOrOptions?: AbortSignal | { signal?: AbortSignal; silent?: boolean },
    maybeOptions?: { silent?: boolean }
  ) => {
    if (!status.running) return;

    const externalSignal = signalOrOptions instanceof AbortSignal
      ? signalOrOptions
      : signalOrOptions?.signal;
    const silent = signalOrOptions instanceof AbortSignal
      ? Boolean(maybeOptions?.silent)
      : Boolean(signalOrOptions?.silent);

    lastFetchControllerRef.current?.abort();
    const controller = new AbortController();
    lastFetchControllerRef.current = controller;
    const activeSignal = controller.signal;
    const requestId = ++fetchSequenceRef.current;

    if (externalSignal) {
      if (externalSignal.aborted) {
        controller.abort(externalSignal.reason);
      } else {
        externalSignal.addEventListener('abort', () => controller.abort(externalSignal.reason), { once: true });
      }
    }

    if (silent) setRefreshing(true);
    else setLoading(true);
    setApiError(false);
    setApiErrorMessage(null);

    try {
      const [proxyData, providerData, clashConfig] = await Promise.all([
        client.getProxies({ signal: activeSignal }),
        client.getProviders({ signal: activeSignal }),
        client.getConfig({ signal: activeSignal }),
      ]);
      await refreshManagedSubscriptions(activeSignal);

      if (!proxyData || activeSignal.aborted || !isMounted.current || requestId !== fetchSequenceRef.current) return;

      setProxies(proxyData);
      setProviders(providerData);
      setCurrentMode(clashConfig.mode);
      setAvailableModes(buildAvailableModes(status.bin_name, clashConfig.mode, clashConfig['mode-list']));

      const initialLatencies: Record<string, number> = {};
      Object.keys(proxyData).forEach(name => {
        const history = proxyData[name].history;
        if (history && history.length > 0) {
          initialLatencies[name] = history[history.length - 1].delay;
        }
      });
      setLatencies(initialLatencies);
    } catch (e) {
      if (activeSignal.aborted || !isMounted.current || requestId !== fetchSequenceRef.current) return;
      console.error('Fetch Data Error:', e);
      setApiError(true);
      setApiErrorMessage(e instanceof Error ? e.message : String(e));
    } finally {
      if (lastFetchControllerRef.current === controller) {
        lastFetchControllerRef.current = null;
      }
      if (!activeSignal.aborted && isMounted.current && requestId === fetchSequenceRef.current) {
        if (silent) setRefreshing(false);
        else setLoading(false);
      }
    }
  }, [client, isMounted, refreshManagedSubscriptions, status.bin_name, status.running]);

  useEffect(() => {
    const controller = new AbortController();
    void fetchInitialData({ signal: controller.signal });
    return () => {
      controller.abort();
      lastFetchControllerRef.current?.abort();
    };
  }, [fetchInitialData]);

  useEffect(() => {
    setAvailableModes(prev => {
      const next = buildAvailableModes(status.bin_name, currentMode, prev);
      if (next.length === prev.length && next.every((mode, index) => mode === prev[index])) {
        return prev;
      }
      return next;
    });
  }, [currentMode, status.bin_name]);

  const handleSelectNode = useCallback(async (groupName: string, nodeName: string) => {
    if (proxies?.[groupName]?.now === nodeName) return;
    try {
      await client.selectProxy(groupName, nodeName);
      if (!isMounted.current) return;
      setProxies(prev => prev ? ({
        ...prev,
        [groupName]: { ...prev[groupName], now: nodeName },
      }) : null);
    } catch (e: unknown) {
      if (isMounted.current) notify(`切换失败: ${e instanceof Error ? e.message : String(e)}`);
    }
  }, [client, isMounted, proxies]);
  
  const handleChangeMode = useCallback(async (mode: string) => {
    if (currentMode === mode) return;
    const oldMode = currentMode;
    setCurrentMode(mode);
    try {
      await client.updateConfig({ mode });
    } catch (e: unknown) {
      if (isMounted.current) {
        setCurrentMode(oldMode);
        notify(`模式切换失败: ${e instanceof Error ? e.message : String(e)}`);
      }
    }
  }, [client, currentMode, isMounted]);

  const handleUpdateProvider = useCallback(async (e: React.MouseEvent, name: string) => {
    e.stopPropagation();
    if (updatingProviders[name]) return;
    markProviderUpdateStart(name);
    try {
      await client.updateProvider(name);
      const providerData = await client.getProviders();
      if (!isMounted.current) return;
      setProviders(providerData);
      notify(`已更新: ${name}`);
    } catch (e: unknown) {
      if (isMounted.current) notify(`更新失败: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      if (isMounted.current) markProviderUpdateEnd(name);
    }
  }, [client, isMounted, markProviderUpdateEnd, markProviderUpdateStart, updatingProviders]);

  const handleSaveSubscription = useCallback(async (currentName: string | null, nextName: string, url: string, type: 'remote' | 'local' = 'remote') => {
    if (status.bin_name !== 'mihomo' && status.bin_name !== 'sing-box') {
      throw new Error('当前核心不支持该操作');
    }

    const action = status.bin_name === 'mihomo'
      ? (currentName ? boxBridge.updateMihomoSubscription(currentName, nextName, url) : boxBridge.addMihomoSubscription(nextName, url))
      : (currentName ? boxBridge.updateSingboxSubscription(currentName, nextName, url, type) : boxBridge.addSingboxSubscription(nextName, url, type));
    const job = await action;
    const providerName = currentName || nextName;
    markProviderUpdateStart(providerName);
    notify(`${getSubscriptionQueuedText(currentName)}已转入后台`);
    void waitForJob(job.job_id)
      .then(async () => {
        try {
          await fetchInitialData({ silent: true });
        } catch {
          await refreshManagedSubscriptions();
        }
        if (isMounted.current) {
          notify(currentName ? '订阅已更新' : '订阅已新增');
        }
      })
      .catch((error: unknown) => {
        if (isMounted.current) notify(`订阅保存失败: ${error instanceof Error ? error.message : String(error)}`);
      })
      .finally(() => {
        if (isMounted.current) markProviderUpdateEnd(providerName);
      });
    return job;
  }, [fetchInitialData, isMounted, markProviderUpdateEnd, markProviderUpdateStart, refreshManagedSubscriptions, status.bin_name]);

  const handleRemoveSubscription = useCallback(async (name: string) => {
    if (status.bin_name !== 'mihomo' && status.bin_name !== 'sing-box') {
      throw new Error('当前核心不支持该操作');
    }

    await (status.bin_name === 'mihomo'
      ? boxBridge.removeMihomoSubscription(name)
      : boxBridge.removeSingboxSubscription(name));
    try {
      await fetchInitialData({ silent: true });
    } catch {
      await refreshManagedSubscriptions();
    }
  }, [fetchInitialData, refreshManagedSubscriptions, status.bin_name]);

  const handleRefreshSubscription = useCallback(async (name: string, url: string) => {
    if (status.bin_name !== 'sing-box') {
      throw new Error('当前核心不支持刷新订阅缓存');
    }

    markProviderUpdateStart(name);
    const job = await boxBridge.updateSingboxSubscription(name, name, url);
    notify('订阅刷新已转入后台');
    void waitForJob(job.job_id)
      .then(async () => {
        try {
          await fetchInitialData({ silent: true });
        } catch {
          await refreshManagedSubscriptions();
        }
        if (isMounted.current) notify('订阅缓存已刷新');
      })
      .catch((error: unknown) => {
        if (isMounted.current) notify(`刷新失败: ${error instanceof Error ? error.message : String(error)}`);
      })
      .finally(() => {
        if (isMounted.current) markProviderUpdateEnd(name);
      });
    return job;
  }, [fetchInitialData, isMounted, markProviderUpdateEnd, markProviderUpdateStart, refreshManagedSubscriptions, status.bin_name]);

  const handleTestProvider = useCallback(async (e: React.MouseEvent, name: string) => {
    e.stopPropagation();
    const ownerKey = `provider:${name}`;
    if (testingOwners[ownerKey]) return;
    const providerNodes = providers?.[name]?.proxies?.map(proxy => proxy.name) || [];
    markTestingStart(ownerKey, providerNodes);
    try {
      await client.healthCheckProvider(name);
      const providerData = await client.getProviders();
      const proxyData = await client.getProxies();

      if (!isMounted.current) return;
      setProviders(providerData);
      setProxies(proxyData);
      setLatencies(prev => {
        const next = { ...prev };
        Object.keys(proxyData).forEach(nodeName => {
          const history = proxyData[nodeName].history;
          if (history && history.length > 0) {
            next[nodeName] = history[history.length - 1].delay;
          }
        });
        return next;
      });
    } catch (e: unknown) {
      if (isMounted.current) notify(`测速失败: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      if (isMounted.current) markTestingEnd(ownerKey, providerNodes);
    }
  }, [client, isMounted, markTestingEnd, markTestingStart, providers, testingOwners]);

  const handleTestGroup = useCallback(async (e: React.MouseEvent, groupName: string, nodes: string[]) => {
    e.stopPropagation();
    const ownerKey = `group:${groupName}`;
    if (testingOwners[ownerKey]) return;
    markTestingStart(ownerKey, nodes);
    try {
      const results = await Promise.all(nodes.map(node => client.testLatency(node).catch(() => 0)));
      if (!isMounted.current) return;
      setLatencies(prev => {
        const next = { ...prev };
        nodes.forEach((node, index) => {
          next[node] = results[index];
        });
        return next;
      });
    } catch {
      if (isMounted.current) notify('测速出错');
    } finally {
      if (isMounted.current) markTestingEnd(ownerKey, nodes);
    }
  }, [client, isMounted, markTestingEnd, markTestingStart, testingOwners]);

  return {
    proxies,
    providers,
    subscriptions,
    latencies,
    loading,
    refreshing,
    apiError,
    apiErrorMessage,
    currentMode,
    availableModes,
    testingOwners,
    testingNodes,
    updatingProviders,
    fetchInitialData,
    handleSelectNode,
    handleChangeMode,
    handleUpdateProvider,
    handleTestProvider,
    handleTestGroup,
    handleSaveSubscription,
    handleRemoveSubscription,
    handleRefreshSubscription,
  };
}
