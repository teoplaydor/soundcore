#pragma once

#include <windows.h>
#include <strmif.h>
#include <uuids.h>
#include <atomic>
#include <vector>

#include "MediaTypes.h"

namespace sc {

class VirtualCameraFilter;

class OutputPin final
    : public IPin
    , public IAMStreamConfig
    , public IKsPropertySet
    , public IAMPushSource
{
public:
    OutputPin(VirtualCameraFilter* parent);
    virtual ~OutputPin();

    // IUnknown
    STDMETHODIMP QueryInterface(REFIID riid, void** out) override;
    STDMETHODIMP_(ULONG) AddRef() override;
    STDMETHODIMP_(ULONG) Release() override;

    // IPin
    STDMETHODIMP Connect(IPin* receivePin, const AM_MEDIA_TYPE* mt) override;
    STDMETHODIMP ReceiveConnection(IPin* connector, const AM_MEDIA_TYPE* mt) override;
    STDMETHODIMP Disconnect() override;
    STDMETHODIMP ConnectedTo(IPin** pin) override;
    STDMETHODIMP ConnectionMediaType(AM_MEDIA_TYPE* mt) override;
    STDMETHODIMP QueryPinInfo(PIN_INFO* info) override;
    STDMETHODIMP QueryDirection(PIN_DIRECTION* dir) override;
    STDMETHODIMP QueryId(LPWSTR* id) override;
    STDMETHODIMP QueryAccept(const AM_MEDIA_TYPE* mt) override;
    STDMETHODIMP EnumMediaTypes(IEnumMediaTypes** out) override;
    STDMETHODIMP QueryInternalConnections(IPin** apPin, ULONG* nPin) override;
    STDMETHODIMP EndOfStream() override;
    STDMETHODIMP BeginFlush() override;
    STDMETHODIMP EndFlush() override;
    STDMETHODIMP NewSegment(REFERENCE_TIME tStart, REFERENCE_TIME tStop, double dRate) override;

    // IAMStreamConfig
    STDMETHODIMP SetFormat(AM_MEDIA_TYPE* mt) override;
    STDMETHODIMP GetFormat(AM_MEDIA_TYPE** mt) override;
    STDMETHODIMP GetNumberOfCapabilities(int* count, int* size) override;
    STDMETHODIMP GetStreamCaps(int index, AM_MEDIA_TYPE** mt, BYTE* scc) override;

    // IKsPropertySet
    STDMETHODIMP Set(REFGUID guidPropSet, DWORD dwPropID,
                     LPVOID pInstanceData, DWORD cbInstanceData,
                     LPVOID pPropData, DWORD cbPropData) override;
    STDMETHODIMP Get(REFGUID guidPropSet, DWORD dwPropID,
                     LPVOID pInstanceData, DWORD cbInstanceData,
                     LPVOID pPropData, DWORD cbPropData,
                     DWORD* pcbReturned) override;
    STDMETHODIMP QuerySupported(REFGUID guidPropSet, DWORD dwPropID, DWORD* pTypeSupport) override;

    // IAMPushSource
    STDMETHODIMP GetPushSourceFlags(ULONG* flags) override;
    STDMETHODIMP SetPushSourceFlags(ULONG flags) override;
    STDMETHODIMP SetStreamOffset(REFERENCE_TIME offset) override;
    STDMETHODIMP GetStreamOffset(REFERENCE_TIME* offset) override;
    STDMETHODIMP GetMaxStreamOffset(REFERENCE_TIME* offset) override;
    STDMETHODIMP SetMaxStreamOffset(REFERENCE_TIME offset) override;
    // IAMLatency
    STDMETHODIMP GetLatency(REFERENCE_TIME* latency) override;

private:
    VirtualCameraFilter* m_parent;
    std::atomic<ULONG> m_ref{0};
    IPin* m_connectedTo = nullptr;
    AM_MEDIA_TYPE m_currentType{};
    std::vector<AdvertisedFormat> m_formats;
};

} // namespace sc
