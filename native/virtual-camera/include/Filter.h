// DirectShow IBaseFilter implementation for the SoundCore virtual camera.
//
// Hand-rolled (no DShow BaseClasses dependency) so we have a single
// in-tree implementation with no external sample dependencies.

#pragma once

#include <windows.h>
#include <unknwn.h>
#include <strmif.h>
#include <atomic>
#include <memory>
#include <vector>
#include <string>

#include "OutputPin.h"
#include "SharedFrameReader.h"

namespace sc {

class VirtualCameraFilter final : public IBaseFilter, public IAMFilterMiscFlags
{
public:
    VirtualCameraFilter();
    virtual ~VirtualCameraFilter();

    // IUnknown
    STDMETHODIMP QueryInterface(REFIID riid, void** out) override;
    STDMETHODIMP_(ULONG) AddRef() override;
    STDMETHODIMP_(ULONG) Release() override;

    // IPersist
    STDMETHODIMP GetClassID(CLSID* clsid) override;

    // IMediaFilter
    STDMETHODIMP Stop() override;
    STDMETHODIMP Pause() override;
    STDMETHODIMP Run(REFERENCE_TIME tStart) override;
    STDMETHODIMP GetState(DWORD msTimeout, FILTER_STATE* state) override;
    STDMETHODIMP SetSyncSource(IReferenceClock* clock) override;
    STDMETHODIMP GetSyncSource(IReferenceClock** clock) override;

    // IBaseFilter
    STDMETHODIMP EnumPins(IEnumPins** out) override;
    STDMETHODIMP FindPin(LPCWSTR id, IPin** out) override;
    STDMETHODIMP QueryFilterInfo(FILTER_INFO* info) override;
    STDMETHODIMP JoinFilterGraph(IFilterGraph* graph, LPCWSTR name) override;
    STDMETHODIMP QueryVendorInfo(LPWSTR* info) override;

    // IAMFilterMiscFlags
    STDMETHODIMP_(ULONG) GetMiscFlags() override;

    FILTER_STATE state() const { return m_state.load(); }
    SharedFrameReader& reader() { return m_reader; }

private:
    std::atomic<ULONG> m_ref{0};
    std::atomic<FILTER_STATE> m_state{State_Stopped};
    IFilterGraph* m_graph = nullptr; // weak ref
    std::wstring m_filterName;
    OutputPin* m_outPin = nullptr;
    IReferenceClock* m_clock = nullptr;
    SharedFrameReader m_reader;
};

} // namespace sc
